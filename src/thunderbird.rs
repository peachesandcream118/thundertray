//! Thunderbird profile discovery across native, Snap, and Flatpak installs.

use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Returns the Thunderbird profile roots in the order they should be checked.
///
/// Keeping this pure makes profile discovery independent of the process home
/// directory and allows callers to test an isolated filesystem tree.
pub fn candidate_profile_roots(home: &Path) -> Vec<PathBuf> {
    vec![
        home.join(".thunderbird"),
        home.join("snap/thunderbird/common/.thunderbird"),
        home.join("snap/thunderbird/current/.thunderbird"),
        home.join(".var/app/org.mozilla.Thunderbird/.thunderbird"),
        home.join(".mozilla-thunderbird"),
    ]
}

fn candidate_profile_roots_for_command(home: &Path, command: &str) -> Vec<PathBuf> {
    let mut roots = candidate_profile_roots(home);
    let command = command.to_ascii_lowercase();
    if command.contains("flatpak") || command.contains("org.mozilla.thunderbird") {
        roots.rotate_left(3);
    } else if command.contains("snap") {
        roots.rotate_left(1);
    }
    roots
}

/// Finds a usable Thunderbird profile below a supplied home directory.
///
/// A profile is only returned when its selected directory exists. The supplied
/// home directory is intentionally explicit so tests and callers can avoid
/// relying on the real user home.
#[cfg(test)]
pub fn detect_thunderbird_profile_from_home(home: &Path) -> Result<PathBuf, ProfileDiscoveryError> {
    detect_thunderbird_profile_from_home_for_command(home, "thunderbird")
}

pub fn detect_thunderbird_profile_from_home_for_command(
    home: &Path,
    command: &str,
) -> Result<PathBuf, ProfileDiscoveryError> {
    let mut tried_profiles_ini = Vec::new();

    for profile_root in candidate_profile_roots_for_command(home, command) {
        let profiles_ini = profile_root.join("profiles.ini");
        tried_profiles_ini.push(profiles_ini.clone());

        let contents = match fs::read_to_string(&profiles_ini) {
            Ok(contents) => contents,
            Err(_) => continue,
        };

        if let Some(profile_path) = profile_candidates_from_ini(&contents, &profile_root)
            .into_iter()
            .find(|candidate| candidate.is_dir())
        {
            return Ok(profile_path);
        }
    }

    Err(ProfileDiscoveryError { tried_profiles_ini })
}

/// Detects a usable Thunderbird profile using the current process home directory.
///
/// This compatibility wrapper preserves the previous no-argument discovery API.
pub fn detect_thunderbird_profile() -> Result<PathBuf, Box<dyn Error>> {
    detect_thunderbird_profile_for_command("thunderbird")
}

pub fn detect_thunderbird_profile_for_command(command: &str) -> Result<PathBuf, Box<dyn Error>> {
    let home = dirs::home_dir().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "Could not determine home directory for Thunderbird profile discovery",
        )
    })?;

    Ok(detect_thunderbird_profile_from_home_for_command(
        &home, command,
    )?)
}

/// Parses a `profiles.ini` document and returns profile paths in selection order.
///
/// The returned order is `[Install*] Default`, `[Profile*] Default=1`, then the
/// first profile with a path. Relative paths are resolved against `profile_root`;
/// absolute paths remain absolute. This function performs no filesystem access.
pub fn profile_candidates_from_ini(contents: &str, profile_root: &Path) -> Vec<PathBuf> {
    let mut install_defaults = Vec::new();
    let mut profiles = Vec::new();
    let mut section = IniSection::Other;

    for raw_line in contents.lines() {
        let line = raw_line.trim();

        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            finish_section(&mut section, &mut profiles);
            section = IniSection::from_name(&line[1..line.len() - 1]);
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };

        let key = key.trim();
        let value = value.trim();

        match &mut section {
            IniSection::Install if key.eq_ignore_ascii_case("Default") && !value.is_empty() => {
                install_defaults.push(value.to_owned());
            }
            IniSection::Profile(profile) => profile.set(key, value),
            IniSection::Install | IniSection::Other => {}
        }
    }

    finish_section(&mut section, &mut profiles);

    let mut candidates = Vec::new();

    for default_path in install_defaults {
        push_unique_candidate(
            &mut candidates,
            resolve_profile_path(profile_root, &default_path, true),
        );
    }

    for profile in profiles.iter().filter(|profile| profile.is_default) {
        if let Some(path) = profile.path.as_deref() {
            push_unique_candidate(
                &mut candidates,
                resolve_profile_path(profile_root, path, profile.is_relative.unwrap_or(true)),
            );
        }
    }

    if let Some(profile) = profiles.iter().find(|profile| profile.path.is_some()) {
        if let Some(path) = profile.path.as_deref() {
            push_unique_candidate(
                &mut candidates,
                resolve_profile_path(profile_root, path, profile.is_relative.unwrap_or(true)),
            );
        }
    }

    candidates
}

/// Error returned when none of the known profile roots yields a usable profile.
#[derive(Debug)]
pub struct ProfileDiscoveryError {
    tried_profiles_ini: Vec<PathBuf>,
}

#[cfg(test)]
impl ProfileDiscoveryError {
    fn tried_profiles_ini(&self) -> &[PathBuf] {
        &self.tried_profiles_ini
    }
}

impl fmt::Display for ProfileDiscoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let paths = self
            .tried_profiles_ini
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");

        write!(
            formatter,
            "No usable Thunderbird profile found. Tried profiles.ini paths: {paths}"
        )
    }
}

impl Error for ProfileDiscoveryError {}

#[derive(Debug)]
enum IniSection {
    Install,
    Profile(ProfileEntry),
    Other,
}

impl IniSection {
    fn from_name(name: &str) -> Self {
        if name.starts_with("Install") {
            Self::Install
        } else if name.starts_with("Profile") {
            Self::Profile(ProfileEntry::default())
        } else {
            Self::Other
        }
    }
}

#[derive(Debug, Default)]
struct ProfileEntry {
    path: Option<String>,
    is_relative: Option<bool>,
    is_default: bool,
}

impl ProfileEntry {
    fn set(&mut self, key: &str, value: &str) {
        if key.eq_ignore_ascii_case("Path") && !value.is_empty() {
            self.path = Some(value.to_owned());
        } else if key.eq_ignore_ascii_case("IsRelative") {
            self.is_relative = Some(value == "1");
        } else if key.eq_ignore_ascii_case("Default") {
            self.is_default = value == "1";
        }
    }
}

fn finish_section(section: &mut IniSection, profiles: &mut Vec<ProfileEntry>) {
    let previous = std::mem::replace(section, IniSection::Other);
    if let IniSection::Profile(profile) = previous {
        profiles.push(profile);
    }
}

fn resolve_profile_path(profile_root: &Path, path: &str, is_relative: bool) -> PathBuf {
    let path = PathBuf::from(path);

    if is_relative && !path.is_absolute() {
        profile_root.join(path)
    } else {
        path
    }
}

fn push_unique_candidate(candidates: &mut Vec<PathBuf>, candidate: PathBuf) {
    if !candidates.iter().any(|existing| existing == &candidate) {
        candidates.push(candidate);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write_profiles_ini(profile_root: &Path, contents: &str) {
        fs::create_dir_all(profile_root).expect("create Thunderbird root");
        fs::write(profile_root.join("profiles.ini"), contents).expect("write profiles.ini");
    }

    fn create_profile(profile_root: &Path, relative_path: &str) -> PathBuf {
        let profile_path = profile_root.join(relative_path);
        fs::create_dir_all(&profile_path).expect("create profile directory");
        profile_path
    }

    #[test]
    fn candidate_roots_cover_supported_layouts_in_priority_order() {
        let temp = tempdir().expect("create temporary home");
        let home = temp.path();

        assert_eq!(
            candidate_profile_roots(home),
            vec![
                home.join(".thunderbird"),
                home.join("snap/thunderbird/common/.thunderbird"),
                home.join("snap/thunderbird/current/.thunderbird"),
                home.join(".var/app/org.mozilla.Thunderbird/.thunderbird"),
                home.join(".mozilla-thunderbird"),
            ]
        );
    }

    #[test]
    fn detects_native_profile_without_real_home() {
        let temp = tempdir().expect("create temporary home");
        let root = temp.path().join(".thunderbird");
        let expected = create_profile(&root, "Profiles/native.default-release");
        write_profiles_ini(
            &root,
            "[Profile0]\nIsRelative=1\nPath=Profiles/native.default-release\nDefault=1\n",
        );

        assert_eq!(
            detect_thunderbird_profile_from_home(temp.path()).expect("detect native profile"),
            expected
        );
    }

    #[test]
    fn falls_back_to_the_first_profile_when_none_is_marked_default() {
        let temp = tempdir().expect("create temporary home");
        let root = temp.path().join(".thunderbird");
        let expected = create_profile(&root, "Profiles/first.default-release");
        create_profile(&root, "Profiles/second.default-release");
        write_profiles_ini(
            &root,
            "[Profile0]\nIsRelative=1\nPath=Profiles/first.default-release\n\
             [Profile1]\nIsRelative=1\nPath=Profiles/second.default-release\n",
        );

        assert_eq!(
            detect_thunderbird_profile_from_home(temp.path()).expect("detect fallback profile"),
            expected
        );
    }

    #[test]
    fn snap_common_has_priority_over_native_profile() {
        let temp = tempdir().expect("create temporary home");
        let snap_root = temp.path().join("snap/thunderbird/common/.thunderbird");
        let native_root = temp.path().join(".thunderbird");
        let expected = create_profile(&snap_root, "Profiles/snap.default-release");
        create_profile(&native_root, "Profiles/native.default-release");
        write_profiles_ini(
            &snap_root,
            "[Profile0]\nIsRelative=1\nPath=Profiles/snap.default-release\nDefault=1\n",
        );
        write_profiles_ini(
            &native_root,
            "[Profile0]\nIsRelative=1\nPath=Profiles/native.default-release\nDefault=1\n",
        );

        assert_eq!(
            detect_thunderbird_profile_from_home_for_command(temp.path(), "snap run thunderbird",)
                .expect("detect Snap profile"),
            expected
        );
    }

    #[test]
    fn explicit_flatpak_command_wins_over_native_profile() {
        let temp = tempdir().expect("create temporary home");
        let native_root = temp.path().join(".thunderbird");
        let flatpak_root = temp
            .path()
            .join(".var/app/org.mozilla.Thunderbird/.thunderbird");
        create_profile(&native_root, "Profiles/native.default-release");
        let expected = create_profile(&flatpak_root, "Profiles/flatpak.default-release");
        write_profiles_ini(
            &native_root,
            "[Profile0]\nPath=Profiles/native.default-release\nDefault=1\n",
        );
        write_profiles_ini(
            &flatpak_root,
            "[Profile0]\nPath=Profiles/flatpak.default-release\nDefault=1\n",
        );

        assert_eq!(
            detect_thunderbird_profile_from_home_for_command(
                temp.path(),
                "flatpak run org.mozilla.Thunderbird",
            )
            .expect("detect Flatpak profile"),
            expected
        );
    }

    #[test]
    fn detects_flatpak_profile() {
        let temp = tempdir().expect("create temporary home");
        let root = temp
            .path()
            .join(".var/app/org.mozilla.Thunderbird/.thunderbird");
        let expected = create_profile(&root, "Profiles/flatpak.default-release");
        write_profiles_ini(
            &root,
            "[Profile0]\nIsRelative=1\nPath=Profiles/flatpak.default-release\nDefault=1\n",
        );

        assert_eq!(
            detect_thunderbird_profile_from_home(temp.path()).expect("detect Flatpak profile"),
            expected
        );
    }

    #[test]
    fn detects_absolute_profile_path() {
        let temp = tempdir().expect("create temporary home");
        let root = temp.path().join(".thunderbird");
        let expected = temp.path().join("outside-thunderbird/absolute-profile");
        fs::create_dir_all(&expected).expect("create absolute profile directory");
        write_profiles_ini(
            &root,
            &format!(
                "[Profile0]\nIsRelative=0\nPath={}\nDefault=1\n",
                expected.display()
            ),
        );

        assert_eq!(
            detect_thunderbird_profile_from_home(temp.path()).expect("detect absolute profile"),
            expected
        );
    }

    #[test]
    fn install_default_has_priority_over_profile_default() {
        let temp = tempdir().expect("create temporary home");
        let root = temp.path().join(".thunderbird");
        let expected = create_profile(&root, "Profiles/install.default-release");
        create_profile(&root, "Profiles/profile.default-release");
        write_profiles_ini(
            &root,
            "[Profile0]\nIsRelative=1\nPath=Profiles/profile.default-release\nDefault=1\n\
             [InstallABCDEF]\nDefault=Profiles/install.default-release\n",
        );

        assert_eq!(
            detect_thunderbird_profile_from_home(temp.path()).expect("detect Install default"),
            expected
        );
    }

    #[test]
    fn missing_profile_reports_every_tried_ini_without_contents() {
        let temp = tempdir().expect("create temporary home");
        let root = temp.path().join(".thunderbird");
        write_profiles_ini(
            &root,
            "[Profile0]\nIsRelative=1\nPath=Profiles/missing.default-release\nDefault=1\nsecret-value\n",
        );

        let error = detect_thunderbird_profile_from_home(temp.path())
            .expect_err("a missing profile directory must not be accepted");
        let message = error.to_string();

        assert_eq!(error.tried_profiles_ini().len(), 5);
        for root in candidate_profile_roots(temp.path()) {
            assert!(message.contains(&root.join("profiles.ini").display().to_string()));
        }
        assert!(!message.contains("secret-value"));
    }
}
