use super::*;

pub(in crate::packages) fn check(input: &mut File) -> Result<()> {
    input.rewind().map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    input
        .take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    valid(&bytes).ok_or_else(|| {
        "A DLL is incomplete or lacks a supported managed image. Waiting for a complete build."
            .into()
    })
}

fn valid(bytes: &[u8]) -> Option<()> {
    if bytes.len() > 16 * 1024 * 1024 || bytes.get(..2)? != b"MZ" {
        return None;
    }
    let word = |at: usize| -> Option<usize> {
        Some(u16::from_le_bytes(bytes.get(at..at.checked_add(2)?)?.try_into().ok()?) as usize)
    };
    let dword = |at: usize| -> Option<usize> {
        Some(u32::from_le_bytes(bytes.get(at..at.checked_add(4)?)?.try_into().ok()?) as usize)
    };
    let pe = dword(60)?;
    if bytes.get(pe..pe.checked_add(4)?)? != b"PE\0\0" {
        return None;
    }
    let count = word(pe.checked_add(6)?)?;
    if !(1..=96).contains(&count) {
        return None;
    }
    let optional_size = word(pe.checked_add(20)?)?;
    let optional = pe.checked_add(24)?;
    let directories = match word(optional)? {
        0x10b => 96,
        0x20b => 112,
        _ => return None,
    };
    if optional_size < directories + 15 * 8 || dword(optional.checked_add(directories - 4)?)? < 15 {
        return None;
    }
    let section_table = optional.checked_add(optional_size)?;
    let section_end = section_table.checked_add(count * 40)?;
    bytes.get(..section_end)?;
    let mut sections = Vec::new();
    for index in 0..count {
        let at = section_table.checked_add(index * 40)?;
        let rva = dword(at + 12)?;
        let size = dword(at + 16)?;
        let offset = dword(at + 20)?;
        if size > 0 {
            if offset < section_end {
                return None;
            }
            bytes.get(offset..offset.checked_add(size)?)?;
        }
        sections.push((rva, offset, size));
    }
    let locate = |rva: usize, size: usize| -> Option<usize> {
        for &(start, offset, length) in &sections {
            if let Some(relative) = rva.checked_sub(start)
                && relative.checked_add(size)? <= length
            {
                return offset.checked_add(relative);
            }
        }
        None
    };
    let cli_directory = optional.checked_add(directories + 14 * 8)?;
    if dword(cli_directory + 4)? < 72 {
        return None;
    }
    let cli = locate(dword(cli_directory)?, 72)?;
    if dword(cli)? < 72 {
        return None;
    }
    let metadata_size = dword(cli + 12)?;
    if metadata_size < 20 {
        return None;
    }
    let metadata = locate(dword(cli + 8)?, metadata_size)?;
    if bytes.get(metadata..metadata.checked_add(4)?)? != b"BSJB" {
        return None;
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_managed_images_are_rejected_before_a_watched_copy_is_accepted() {
        let mut bytes = vec![0u8; 1024];
        bytes[..2].copy_from_slice(b"MZ");
        bytes[128..132].copy_from_slice(b"PE\0\0");
        for (at, value) in [
            (60, 128u32),
            (244, 16),
            (360, 8192),
            (364, 72),
            (388, 8192),
            (392, 512),
            (396, 512),
            (512, 72),
            (520, 8320),
            (524, 64),
        ] {
            bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
        for (at, value) in [(134, 1u16), (148, 224), (152, 0x10b)] {
            bytes[at..at + 2].copy_from_slice(&value.to_le_bytes());
        }
        bytes[640..644].copy_from_slice(b"BSJB");
        assert!(valid(&bytes).is_some());
        for length in [0, 2, 63, 127, 256, 512, 640, 1023] {
            assert!(
                valid(&bytes[..length]).is_none(),
                "accepted truncated image at {length}"
            );
        }
        bytes[360..364].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(valid(&bytes).is_none());
        assert!(valid(b"inert bytes are not a managed DLL").is_none());
    }
}
