use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::config::{validate_group_name, GroupConfig, PatternKind};
use super::error::GroupConfigError;

/// Looked for in the repo root when no config path is given.
pub const DEFAULT_CONFIG_FILE_NAME: &str = ".code_reviewer.toml";

/// Group options from the command line, or from a UI form with the same fields.
///
/// | Flag                                | Field               |
/// |-------------------------------------|---------------------|
/// | `--group-config <path>`             | `group_config_path` |
/// | `--group-include <group>=<pattern>` | `include`           |
/// | `--group-exclude <group>=<pattern>` | `exclude`           |
///
/// The include and exclude flags repeat, and add to what the config file
/// defines. `<group>` is a group name, created when missing. Only the first `=`
/// splits the flag.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupFlags {
    /// A config file to use instead of the default one in the repo root.
    #[serde(default)]
    pub group_config_path: Option<String>,
    /// Each entry is `<group>=<pattern>`.
    #[serde(default)]
    pub include: Vec<String>,
    /// Each entry is `<group>=<pattern>`.
    #[serde(default)]
    pub exclude: Vec<String>,
}

/// Read the config file, then add the flags on top.
///
/// - With `group_config_path`, the file must exist. A relative path is
///   relative to the working directory of the process.
/// - Without it, `.code_reviewer.toml` in the repo root is used when present,
///   and no file means every file is in the default group.
pub fn load_group_config(
    repo_path: &str,
    flags: &GroupFlags,
) -> Result<GroupConfig, GroupConfigError> {
    let mut config = match &flags.group_config_path {
        Some(path) => {
            read_config_file(Path::new(path))?.ok_or_else(|| read_failed(path, "file not found"))?
        }
        None => read_config_file(&Path::new(repo_path).join(DEFAULT_CONFIG_FILE_NAME))?
            .unwrap_or_default(),
    };
    apply_flags(&mut config, flags)?;
    Ok(config)
}

/// Add the include and exclude flags to the config.
pub fn apply_flags(config: &mut GroupConfig, flags: &GroupFlags) -> Result<(), GroupConfigError> {
    for flag in &flags.include {
        let (group_name, pattern) = parse_group_flag(flag)?;
        config.add_pattern(&group_name, PatternKind::Include, pattern);
    }
    for flag in &flags.exclude {
        let (group_name, pattern) = parse_group_flag(flag)?;
        config.add_pattern(&group_name, PatternKind::Exclude, pattern);
    }
    Ok(())
}

/// Split `data=lib/db/**` into the group name and the pattern.
fn parse_group_flag(flag: &str) -> Result<(String, String), GroupConfigError> {
    let invalid = |reason: &str| GroupConfigError::InvalidFlag {
        flag: flag.to_string(),
        reason: reason.to_string(),
    };
    let (group, pattern) = flag
        .split_once('=')
        .ok_or_else(|| invalid("expected <group>=<pattern>"))?;
    let pattern = pattern.trim();
    if pattern.is_empty() {
        return Err(invalid("pattern is empty"));
    }
    let group_name = group.trim();
    validate_group_name(group_name).map_err(|error| invalid(&error.to_string()))?;
    Ok((group_name.to_string(), pattern.to_string()))
}

fn read_config_file(path: &Path) -> Result<Option<GroupConfig>, GroupConfigError> {
    let display_path = path.display().to_string();
    match fs::read_to_string(path) {
        Ok(contents) => GroupConfig::from_toml_str(&contents, &display_path).map(Some),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(read_failed(&display_path, &error.to_string())),
    }
}

fn read_failed(path: &str, message: &str) -> GroupConfigError {
    GroupConfigError::ReadFileFailed {
        path: path.to_string(),
        message: message.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flags(include: &[&str], exclude: &[&str]) -> GroupFlags {
        GroupFlags {
            group_config_path: None,
            include: include.iter().map(|flag| flag.to_string()).collect(),
            exclude: exclude.iter().map(|flag| flag.to_string()).collect(),
        }
    }

    #[test]
    fn test_flags_create_groups_and_keep_equals_signs_in_patterns() {
        let mut config = GroupConfig::default();

        apply_flags(
            &mut config,
            &flags(&["data=lib/db/**", "data=regex:a=b"], &["data=**/*_test.*"]),
        )
        .unwrap();

        let data = &config.groups["data"];
        assert_eq!(data.include, ["lib/db/**", "regex:a=b"]);
        assert_eq!(data.exclude, ["**/*_test.*"]);
    }

    #[test]
    fn test_flags_add_to_groups_from_the_file() {
        let mut config =
            GroupConfig::from_toml_str("[groups.tests]\ninclude = [\"test/**\"]", "test").unwrap();

        apply_flags(&mut config, &flags(&["tests=**/*_spec.*"], &[])).unwrap();

        assert_eq!(config.groups["tests"].include, ["test/**", "**/*_spec.*"]);
    }

    #[test]
    fn test_malformed_flags_are_rejected() {
        for bad in ["core", "=x", "core=", "core data=x", "core/data=x"] {
            let result = apply_flags(&mut GroupConfig::default(), &flags(&[bad], &[]));
            assert!(
                matches!(result, Err(GroupConfigError::InvalidFlag { .. })),
                "{bad}"
            );
        }
    }

    #[test]
    fn test_load_reads_the_default_file_in_the_repo_root() {
        let repo = tempfile::tempdir().unwrap();
        fs::write(
            repo.path().join(DEFAULT_CONFIG_FILE_NAME),
            "[groups.docs]\ninclude = [\"docs/**\"]",
        )
        .unwrap();

        let config =
            load_group_config(repo.path().to_str().unwrap(), &flags(&["docs=*.md"], &[])).unwrap();

        assert_eq!(config.groups["docs"].include, ["docs/**", "*.md"]);
    }

    #[test]
    fn test_load_without_any_file_gives_only_the_flag_groups() {
        let repo = tempfile::tempdir().unwrap();
        let repo_path = repo.path().to_str().unwrap();

        assert!(load_group_config(repo_path, &GroupFlags::default())
            .unwrap()
            .groups
            .is_empty());
        let config = load_group_config(repo_path, &flags(&["docs=docs/**"], &[])).unwrap();
        assert_eq!(config.groups.len(), 1);
    }

    #[test]
    fn test_load_requires_an_explicit_config_path_to_exist() {
        let repo = tempfile::tempdir().unwrap();
        let missing = repo.path().join("missing.toml");
        let flags = GroupFlags {
            group_config_path: Some(missing.display().to_string()),
            ..GroupFlags::default()
        };

        let result = load_group_config(repo.path().to_str().unwrap(), &flags);

        assert!(matches!(
            result,
            Err(GroupConfigError::ReadFileFailed { .. })
        ));
    }

    #[test]
    fn test_load_reports_which_file_failed_to_parse() {
        let repo = tempfile::tempdir().unwrap();
        fs::write(repo.path().join(DEFAULT_CONFIG_FILE_NAME), "[groups.a").unwrap();

        let error =
            load_group_config(repo.path().to_str().unwrap(), &GroupFlags::default()).unwrap_err();

        assert!(error.to_string().contains(DEFAULT_CONFIG_FILE_NAME));
    }
}
