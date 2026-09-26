use serde::{Deserialize, Serialize};

type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum Layout {
    StarframeLuaZip {},
    #[serde(rename_all = "camelCase")]
    StarframeManagedZip {
        root: String,
        entry_assembly: String,
        entry_type: String,
    },
}

fn ensure(condition: bool, message: &str) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("Invalid local metadata: {message}."))
    }
}

fn text(value: &str, limit: usize) -> Result<()> {
    ensure(
        !value.trim().is_empty() && value.len() <= limit && !value.chars().any(char::is_control),
        "empty, oversized or control-character text",
    )
}

pub(crate) fn id(value: &str) -> Result<()> {
    ensure(
        !value.is_empty()
            && value.len() <= 128
            && value.bytes().enumerate().all(|(i, c)| {
                c.is_ascii_lowercase() || c.is_ascii_digit() || (i > 0 && b"._-".contains(&c))
            }),
        "IDs must start with a lowercase ASCII letter or digit and contain at most 128 letters, digits, dots, underscores or hyphens",
    )
}

pub(crate) fn validate_layout(layout: &Layout) -> Result<()> {
    if let Layout::StarframeManagedZip {
        root,
        entry_assembly,
        entry_type,
    } = layout
    {
        if !root.is_empty() {
            crate::runtime_contract::relative_path(root)?;
        }
        crate::runtime_contract::relative_path(entry_assembly)?;
        ensure(
            entry_assembly.ends_with(".dll"),
            "managed entry must be a DLL",
        )?;
        text(entry_type, 256)?;
        ensure(
            entry_type.split('.').all(|part| {
                !part.is_empty()
                    && part.bytes().enumerate().all(|(i, c)| {
                        c.is_ascii_alphabetic() || c == b'_' || (i > 0 && c.is_ascii_digit())
                    })
            }),
            "invalid managed entry type",
        )?;
    }
    Ok(())
}
