use super::*;

pub(crate) fn supported_files(files: &[PreparedFile]) -> Result<()> {
    if files.is_empty() || files.len() > crate::runtime_contract::MAX_FILES_PER_MOD {
        return Err(format!(
            "This package contains {} runtime files; supported packages contain 1–1,024. Ask the author to split or reduce the package.",
            files.len()
        ));
    }
    let mut total = 0u64;
    for file in files {
        if file.size_bytes > 64 * 1024 * 1024 {
            return Err(format!(
                "{} exceeds the runtime's 64 MiB file limit. Ask the author to reduce it.",
                file.path
            ));
        }
        if file.path.to_ascii_lowercase().ends_with(".dll") && file.size_bytes > 16 * 1024 * 1024 {
            return Err(format!(
                "{} exceeds the runtime's 16 MiB managed assembly limit. Ask the author to split it.",
                file.path
            ));
        }
        total += file.size_bytes;
    }
    if total > crate::runtime_contract::MAX_ACTIVATION_BYTES {
        return Err("This package exceeds the runtime's 256 MiB activation limit. Ask the author to split or reduce it.".into());
    }
    Ok(())
}

pub(crate) fn layout(files: &[PreparedFile], layout: &Layout) -> Result<()> {
    supported_files(files)?;
    if matches!(layout, Layout::StarframeLuaZip {}) {
        if files.is_empty() || files.iter().any(|f| !lua_path(&f.path)) {
            return Err("Lua overlays require only .lua files under LJ/lua; AI and map content are unsupported.".into());
        }
        return Ok(());
    }
    let Layout::StarframeManagedZip {
        root,
        entry_assembly,
        ..
    } = layout
    else {
        unreachable!()
    };
    let entry = if root.is_empty() {
        entry_assembly.clone()
    } else {
        format!("{root}/{entry_assembly}")
    };
    if !files
        .iter()
        .any(|file| file.path == entry && file.size_bytes > 0)
    {
        return Err(format!(
            "Unsupported package layout: approved entry {entry} is missing or empty. Ask the curator to correct the release."
        ));
    }
    Ok(())
}

pub(crate) fn lua_path(path: &str) -> bool {
    let path = path.to_ascii_lowercase();
    path.starts_with("lj/lua/")
        && path.ends_with(".lua")
        && !path.split('/').any(|part| part == "ai")
        && crate::runtime_contract::relative_path(&path).is_ok()
}

pub(super) fn extract(
    archive: &Path,
    content: &Path,
    artifact: &Artifact,
    cancel: &Cancel,
) -> Result<Prepared> {
    let mut input = read_file(archive)?;
    let (size, hash) = digest(&mut input, artifact.size_bytes, cancel)?;
    if size != artifact.size_bytes || hash != artifact.sha256 {
        return Err("Package bytes do not match the approved SHA-256 and size. Retry or contact the curator.".into());
    }
    input.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    let count = preflight_zip(&mut input)?;
    let mut archive =
        zip::ZipArchive::new(input).map_err(|e| format!("Unsupported or damaged ZIP: {e}"))?;
    if archive.len() != count || archive.has_overlapping_files().map_err(|e| e.to_string())? {
        return Err("ZIP has duplicate names or overlapping entry data.".into());
    }
    let mut names: BTreeMap<String, (String, bool)> = BTreeMap::new();
    let mut total = 0u64;
    for index in 0..archive.len() {
        cancel.check()?;
        let file = archive
            .by_index(index)
            .map_err(|e| format!("Cannot read ZIP entry: {e}"))?;
        let name = file.name().strip_suffix('/').unwrap_or(file.name());
        let normalized = crate::runtime_contract::relative_path(name)?;
        let is_directory = file.is_dir();
        if file.encrypted()
            || !matches!(
                file.compression(),
                zip::CompressionMethod::Stored | zip::CompressionMethod::Deflated
            )
        {
            return Err("Only unencrypted Stored or Deflate ZIP entries are supported.".into());
        }
        if let Some(mode) = file.unix_mode() {
            let kind = mode & 0o170000;
            if !matches!(kind, 0 | 0o100000 | 0o040000)
                || (kind == 0o040000 && !is_directory)
                || (kind == 0o100000 && is_directory)
            {
                return Err("ZIP links and special files are not supported.".into());
            }
        }
        total = total
            .checked_add(file.size())
            .ok_or("ZIP expanded size overflow.")?;
        if file.size() > MAX_FILE_BYTES
            || total > MAX_EXPANDED_BYTES
            || (is_directory && file.size() != 0)
        {
            return Err("ZIP exceeds the 512 MiB per-file or 2 GiB expanded-size limit.".into());
        }
        if names
            .insert(normalized, (name.into(), is_directory))
            .is_some()
        {
            return Err("ZIP contains duplicate or case-colliding paths.".into());
        }
    }
    let mut directories = names.clone();
    for (original, _) in names.values() {
        let mut prefix = String::new();
        let parts: Vec<_> = original.split('/').collect();
        for part in &parts[..parts.len() - 1] {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(part);
            let lower = prefix.to_ascii_lowercase();
            if let Some((existing, directory)) = directories.get(&lower)
                && (!directory || existing != &prefix)
            {
                return Err("ZIP has a file/directory or directory-case collision.".into());
            }
            directories.insert(lower, (prefix.clone(), true));
        }
    }
    crate::space::require(content, total)?;
    let mut directory = Directory::open(content)?;
    let mut files = Vec::new();
    for index in 0..archive.len() {
        cancel.check()?;
        let mut entry = archive.by_index(index).map_err(|e| e.to_string())?;
        let name = entry.name().to_owned();
        if entry.is_dir() {
            directory.directory(name.trim_end_matches('/'))?;
            continue;
        }
        if let Some((parent, _)) = name.rsplit_once('/') {
            directory.directory(parent)?;
        }
        let target = content.join(&name);
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
            .map_err(|e| format!("Cannot create extracted file: {e}"))?;
        let mut hash = Sha256::new();
        let mut size = 0u64;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            cancel.check()?;
            let count = entry
                .read(&mut buffer)
                .map_err(|e| format!("ZIP integrity check failed: {e}"))?;
            if count == 0 {
                break;
            }
            size += count as u64;
            if size > entry.size() || size > MAX_FILE_BYTES {
                return Err("ZIP entry expands beyond its declared size.".into());
            }
            output
                .write_all(&buffer[..count])
                .map_err(|e| format!("Cannot write extracted package: {e}"))?;
            hash.update(&buffer[..count]);
        }
        if size != entry.size() {
            return Err("ZIP entry is incomplete.".into());
        }
        output.sync_all().map_err(|e| e.to_string())?;
        files.push(PreparedFile {
            path: name,
            sha256: format!("{:x}", hash.finalize()),
            size_bytes: size,
        });
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    layout(&files, &artifact.layout)?;
    Ok(Prepared {
        hash: artifact.sha256.clone(),
        files,
    })
}

pub(super) fn digest(input: &mut File, limit: u64, cancel: &Cancel) -> Result<(u64, String)> {
    let mut hash = Sha256::new();
    let mut size = 0;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        cancel.check()?;
        let count = input.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        size += count as u64;
        if size > limit {
            return Err("Package file exceeds its expected size.".into());
        }
        hash.update(&buffer[..count]);
    }
    Ok((size, format!("{:x}", hash.finalize())))
}

// Bound central-directory allocation before the ZIP library parses it. ZIP64 and
// split archives are unnecessary within this package format's 2 GiB limit.
pub(super) fn preflight_zip(input: &mut File) -> Result<usize> {
    let length = input.metadata().map_err(|e| e.to_string())?.len();
    let tail_length = length.min(65_557) as usize;
    input
        .seek(SeekFrom::End(-(tail_length as i64)))
        .map_err(|e| e.to_string())?;
    let mut tail = vec![0; tail_length];
    input.read_exact(&mut tail).map_err(|e| e.to_string())?;
    let end = tail
        .windows(4)
        .rposition(|p| p == b"PK\x05\x06")
        .ok_or("ZIP end record is missing.")?;
    if end + 22 > tail.len() {
        return Err("ZIP end record is truncated.".into());
    }
    let u16_at = |bytes: &[u8], at| u16::from_le_bytes([bytes[at], bytes[at + 1]]) as usize;
    let u32_at =
        |bytes: &[u8], at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as u64;
    let footer = &tail[end..];
    let count = u16_at(footer, 10);
    let size = u32_at(footer, 12);
    let start = u32_at(footer, 16);
    if count == 0
        || count > MAX_ENTRIES
        || u16_at(footer, 8) != count
        || u16_at(footer, 4) != 0
        || u16_at(footer, 6) != 0
        || size > 4 * 1024 * 1024
        || start + size != length - tail_length as u64 + end as u64
        || end + 22 + u16_at(footer, 20) != tail.len()
    {
        return Err("ZIP exceeds entry/metadata limits or uses an unsupported ZIP64, split or trailing-data layout.".into());
    }
    input
        .seek(SeekFrom::Start(start))
        .map_err(|e| e.to_string())?;
    let mut headers = vec![0; size as usize];
    input.read_exact(&mut headers).map_err(|e| e.to_string())?;
    let mut cursor = 0;
    for _ in 0..count {
        if cursor + 46 > headers.len() || &headers[cursor..cursor + 4] != b"PK\x01\x02" {
            return Err("Invalid ZIP central directory.".into());
        }
        let header = &headers[cursor..];
        let name_size = u16_at(header, 28);
        let extra_size = u16_at(header, 30);
        let end = cursor + 46 + name_size + extra_size + u16_at(header, 32);
        if end > headers.len() || u16_at(header, 34) != 0 || u32_at(header, 38) & 0x400 != 0 {
            return Err("ZIP has an invalid entry or a reparse-point attribute.".into());
        }
        let mut extra = &headers[cursor + 46 + name_size..cursor + 46 + name_size + extra_size];
        while !extra.is_empty() {
            if extra.len() < 4 {
                return Err("Invalid ZIP extra field.".into());
            }
            let kind = u16_at(extra, 0);
            let size = u16_at(extra, 2);
            if size + 4 > extra.len() || matches!(kind, 0x0001 | 0x000d | 0x756e) {
                return Err("ZIP64 and Unix link metadata are not supported.".into());
            }
            extra = &extra[4 + size..];
        }
        cursor = end;
    }
    if cursor != headers.len() {
        return Err("ZIP central-directory count does not match its data.".into());
    }
    input.rewind().map_err(|e| e.to_string())?;
    Ok(count)
}
