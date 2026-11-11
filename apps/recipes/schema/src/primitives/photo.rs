//! Photo file names. A photo is a copy of the picked file in the app's data folder, named by what
//! is in it, so the same picture added twice is stored once and a name can never point outside the
//! folder. The recipes only keep these names.

use std::path::Path;

/// What can be shown: the picture formats the toolkit decodes.
pub const EXTENSIONS: [&str; 5] = ["jpg", "png", "webp", "gif", "bmp"];

/// The extension a file's photo would get (`jpeg` is `jpg`), or `None` if it is not a picture we
/// show.
pub fn extension(path: &Path) -> Option<&'static str> {
    let e = path.extension()?.to_str()?.to_ascii_lowercase();
    let e = if e == "jpeg" { "jpg".to_string() } else { e };
    EXTENSIONS.into_iter().find(|x| *x == e)
}

/// FNV-1a: tiny, and the same on every platform and run, so a name can be recomputed.
fn fnv(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// The stored name of a photo with these contents: sixteen hex digits and the extension.
pub fn name_for(bytes: &[u8], ext: &str) -> String {
    format!("{:016x}.{ext}", fnv(bytes))
}

/// Whether `name` is one this app makes: nothing else is ever read from the photos folder.
pub fn is_valid_name(name: &str) -> bool {
    let Some((stem, ext)) = name.split_once('.') else {
        return false;
    };
    stem.len() == 16
        && stem
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        && EXTENSIONS.contains(&ext)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn only_pictures_have_an_extension_and_jpeg_is_jpg() {
        let ext = |p: &str| extension(&PathBuf::from(p));
        assert_eq!(ext("a/b/Dinner.JPG"), Some("jpg"));
        assert_eq!(ext("x.jpeg"), Some("jpg"));
        assert_eq!(ext("x.png"), Some("png"));
        assert_eq!(ext("notes.txt"), None);
        assert_eq!(ext("script.sh"), None);
        assert_eq!(ext("no_extension"), None);
    }

    #[test]
    fn the_same_contents_get_the_same_name_and_different_ones_do_not() {
        assert_eq!(name_for(b"abc", "png"), name_for(b"abc", "png"));
        assert_ne!(name_for(b"abc", "png"), name_for(b"abd", "png"));
        assert!(is_valid_name(&name_for(b"anything", "jpg")));
    }

    #[test]
    fn a_name_that_is_not_ours_is_refused() {
        for bad in [
            "",
            "photo.png",
            "../secret.png",
            "0123456789abcdef",
            "0123456789abcdeg.png",
            "0123456789abcdef.exe",
            "/etc/passwd",
            "0123456789abcdef.png/x",
            "0123456789ABCDEF.png",
        ] {
            assert!(!is_valid_name(bad), "{bad}");
        }
    }
}
