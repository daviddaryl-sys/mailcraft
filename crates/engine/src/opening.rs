//! Opening an attachment with the program the computer opens that kind of file with (#126).
//!
//! The engine never opens anything: a message is hostile, and so is the name its sender gave a
//! file. This is what the desktop host and the window go by before they do:
//! - [`verdict`]: programs, scripts, shortcuts, installers and disk images are never opened
//!   (they can run as soon as they open); Office files with macros and web pages ask first
//!   (macros and scripts are what they are for); anything else opens.
//! - [`file_name`]: the name the file is written under, from the attachment's own name, made
//!   safe to put in a folder: no folders of its own, nothing hidden, nothing that reads as an
//!   option, no name Windows keeps for itself, not too long.
//!
//! `mail.openAttachment` (`cmd::mail`) answers with both and opens nothing itself: the window
//! does the opening, through the host (`Services::open_file`), and agents may not ask for it.

/// What may be done with an attachment the owner asked to open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Open it.
    Open,
    /// An Office file that can carry macros: ask first.
    Macros,
    /// A web page or a drawing that can carry scripts, which open in the browser: ask first.
    Scripts,
    /// A program, a script, a shortcut, an installer or a disk image: never opened from mail.
    Blocked,
}

impl Verdict {
    /// Whether the owner is asked before it opens.
    pub fn asks(self) -> bool {
        matches!(self, Verdict::Macros | Verdict::Scripts)
    }
}

/// Kinds of file that run (or run what is in them) when they are opened. Windows, macOS, Linux
/// and the BSDs together; a file of one of these is saved, never opened.
pub const BLOCKED: &[&str] = &[
    // Windows programs, libraries, control panel items, screen savers, installers and updates.
    "exe",
    "com",
    "scr",
    "pif",
    "cpl",
    "dll",
    "ocx",
    "sys",
    "drv",
    "msi",
    "msp",
    "mst",
    "msu",
    "msix",
    "msixbundle",
    "appx",
    "appxbundle",
    "application",
    "appref-ms",
    "appinstaller",
    "gadget",
    "xbap",
    "xll",
    "diagcab",
    "msc",
    "ins",
    "isp",
    "inf",
    "reg",
    "scf",
    // Windows scripts and their hosts.
    "bat",
    "cmd",
    "ps1",
    "ps1xml",
    "ps2",
    "ps2xml",
    "psc1",
    "psc2",
    "psd1",
    "psm1",
    "pssc",
    "msh",
    "msh1",
    "msh2",
    "mshxml",
    "msh1xml",
    "msh2xml",
    "vb",
    "vbe",
    "vbs",
    "vbp",
    "js",
    "jse",
    "ws",
    "wsc",
    "wsf",
    "wsh",
    "wsb",
    "hta",
    "htc",
    "sct",
    "shb",
    "shs",
    "chm",
    "hlp",
    "cnt",
    "mde",
    "mda",
    "mdb",
    "mdt",
    "mdw",
    "mdz",
    "ade",
    "adp",
    "accde",
    "bas",
    "fxp",
    "prg",
    "crt",
    "cer",
    "der",
    "udl",
    "printerexport",
    "settingcontent-ms",
    "theme",
    "themepack",
    "webpnp",
    "website",
    // Shortcuts and links that point somewhere else.
    "lnk",
    "url",
    "library-ms",
    "search-ms",
    "searchconnector-ms",
    "glk",
    "xnk",
    "jnlp",
    // Disk images, which open as a drive with whatever is in them.
    "iso",
    "img",
    "vhd",
    "vhdx",
    "dmg",
    "sparseimage",
    "sparsebundle",
    // Java, Python, Perl, Ruby, PHP, Lua and the shells.
    "jar",
    "class",
    "py",
    "pyc",
    "pyo",
    "pyw",
    "pyz",
    "pyzw",
    "pl",
    "pm",
    "rb",
    "php",
    "lua",
    "tcl",
    "sh",
    "bash",
    "zsh",
    "ksh",
    "csh",
    "tcsh",
    "fish",
    "command",
    "tool",
    "run",
    "bin",
    "out",
    "elf",
    // macOS programs, installers, scripts and workflows.
    "app",
    "pkg",
    "mpkg",
    "scpt",
    "scptd",
    "applescript",
    "workflow",
    "action",
    "terminal",
    "webloc",
    "inetloc",
    "fileloc",
    "dylib",
    "kext",
    "prefpane",
    "osax",
    "plugin",
    "bundle",
    // Linux and BSD programs, installers and launchers.
    "desktop",
    "appimage",
    "flatpak",
    "flatpakref",
    "snap",
    "deb",
    "rpm",
    "so",
    "ko",
];

/// Office files that can carry macros.
pub const MACROS: &[&str] = &[
    "docm", "dotm", "xlsm", "xltm", "xlam", "xlsb", "xla", "xlm", "pptm", "potm", "ppam", "ppsm", "sldm", "accdb", "one", "onepkg", "vsdm", "vstm",
    "vssm",
];

/// Web pages and drawings that open in the browser, where what is in them runs.
pub const SCRIPTS: &[&str] = &["htm", "html", "xhtml", "shtml", "mht", "mhtml", "svg", "svgz", "xht", "xml", "xsl"];

/// Names Windows keeps for devices: a file with one of these before its first dot is no file.
const RESERVED: &[&str] = &[
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8", "com9", "com¹", "com²", "com³", "lpt1", "lpt2",
    "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9", "lpt¹", "lpt²", "lpt³", "conin$", "conout$",
];

/// The longest a file name is made, in characters (a folder's path takes the rest).
pub const MAX_NAME: usize = 120;

/// The kind of a file name: what is after its last dot, in lower case (empty when nothing).
pub fn extension(name: &str) -> String {
    let name = name.trim_end_matches(['.', ' ']);
    match name.rsplit_once('.') {
        Some((_, ext)) => ext.trim().to_lowercase(),
        None => String::new(),
    }
}

/// What may be done with an attachment of this name when the owner asks to open it. It is read
/// from the name the file would be written under ([`file_name`]), which is what the computer
/// goes by when it chooses the program.
pub fn verdict(name: &str) -> Verdict {
    let safe = file_name(name);
    let ext = extension(&safe);
    if ext.is_empty() {
        // No kind to choose a program by: the computer guesses from what is in it.
        return Verdict::Blocked;
    }
    if BLOCKED.contains(&ext.as_str()) {
        Verdict::Blocked
    } else if MACROS.contains(&ext.as_str()) {
        Verdict::Macros
    } else if SCRIPTS.contains(&ext.as_str()) {
        Verdict::Scripts
    } else {
        Verdict::Open
    }
}

/// What the owner is told when an attachment isn't opened because of its kind.
pub fn blocked_message(name: &str) -> String {
    let shown: String = file_name(name).chars().take(60).collect();
    format!(
        "MailCraft doesn't open {shown}: a program, a script or a file of no known kind sent in mail can harm the computer. If you trust it, save it with Save As."
    )
}

/// The name an attachment is written under to be opened: only the last part of a path the
/// sender put in its name; no control characters or characters a folder can't have; no leading
/// dots, dashes or spaces (hidden files, options); no trailing dots or spaces (Windows drops
/// them, and with them what the name said it was); not a name Windows keeps for a device; at
/// most [`MAX_NAME`] characters with its kind kept. Never empty.
pub fn file_name(name: &str) -> String {
    let leaf = name.rsplit(['/', '\\']).next().unwrap_or_default();
    let cleaned: String = leaf.chars().map(|c| if c.is_control() || "\\/:*?\"<>|".contains(c) || is_invisible(c) { '_' } else { c }).collect();
    let trimmed = cleaned.trim_start_matches(['.', '-', ' ', '_', '~']).trim_end_matches(['.', ' ']);
    let mut out = shorten(trimmed);
    let stem_lower = out.split('.').next().unwrap_or_default().trim_end().to_lowercase();
    if RESERVED.contains(&stem_lower.as_str()) {
        out = format!("_{out}");
    }
    if out.is_empty() { "attachment".to_string() } else { out }
}

/// Characters that change how a name looks without being seen (right-to-left overrides that turn
/// `gpj.exe` into what reads as `exe.jpg`, zero-width marks).
fn is_invisible(c: char) -> bool {
    matches!(c, '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}' | '\u{feff}' | '\u{00ad}')
}

/// At most [`MAX_NAME`] characters, the kind kept when there is a short one.
fn shorten(name: &str) -> String {
    if name.chars().count() <= MAX_NAME {
        return name.to_string();
    }
    let ext = match name.rsplit_once('.') {
        Some((_, ext)) if !ext.is_empty() && ext.chars().count() <= 16 => format!(".{ext}"),
        _ => String::new(),
    };
    let keep = MAX_NAME.saturating_sub(ext.chars().count());
    let stem: String = name.chars().take(keep).collect();
    format!("{}{ext}", stem.trim_end_matches(['.', ' ']))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hostile_names_are_made_safe() {
        let cases = [
            ("report.pdf", "report.pdf"),
            ("../../etc/passwd", "passwd"),
            ("..\\..\\Windows\\System32\\evil.pdf", "evil.pdf"),
            ("C:\\Users\\x\\a.pdf", "a.pdf"),
            (".bashrc", "bashrc"),
            ("-rf.txt", "rf.txt"),
            ("--help", "help"),
            ("...", "attachment"),
            ("", "attachment"),
            ("   ", "attachment"),
            ("a\0b\r\n.txt", "a_b__.txt"),
            ("what?.txt", "what_.txt"),
            ("CON.txt", "_CON.txt"),
            ("nul", "_nul"),
            ("Lpt1.tar.gz", "_Lpt1.tar.gz"),
            ("console.txt", "console.txt"),
            ("invoice.exe.", "invoice.exe"),
            ("invoice.exe   ", "invoice.exe"),
            ("photo\u{202e}gpj.exe", "photo_gpj.exe"),
        ];
        for (name, want) in cases {
            assert_eq!(file_name(name), want, "{name:?}");
        }
        let long = format!("{}.pdf", "a".repeat(500));
        let short = file_name(&long);
        assert!(short.chars().count() <= MAX_NAME && short.ends_with(".pdf"), "{short}");
        let wide = "é".repeat(400);
        assert!(file_name(&wide).chars().count() <= MAX_NAME);
        for name in ["", "/", "\\", "././.", "a/b/c/", "\u{feff}", "COM1", "con.", "\u{1F600}.pdf"] {
            let safe = file_name(name);
            assert!(!safe.is_empty() && !safe.contains(['/', '\\']) && !safe.starts_with(['.', '-']), "{name:?} -> {safe:?}");
        }
    }

    #[test]
    fn programs_are_never_opened() {
        for name in [
            "setup.exe",
            "run.BAT",
            "x.cmd",
            "a.com",
            "s.scr",
            "i.msi",
            "p.ps1",
            "v.vbs",
            "j.js",
            "k.jar",
            "s.sh",
            "Thing.app",
            "short.lnk",
            "disk.iso",
            "invoice.pdf.exe",
            "invoice.exe.",
            "photo\u{202e}gpj.exe",
            "noextension",
            "../x.exe",
        ] {
            assert_eq!(verdict(name), Verdict::Blocked, "{name}");
        }
    }

    #[test]
    fn macros_and_pages_ask_and_the_rest_opens() {
        for name in ["budget.xlsm", "Letter.DOCM", "deck.pptm"] {
            assert_eq!(verdict(name), Verdict::Macros, "{name}");
        }
        for name in ["page.html", "x.htm", "logo.svg"] {
            assert_eq!(verdict(name), Verdict::Scripts, "{name}");
        }
        for name in ["report.pdf", "photo.JPG", "notes.txt", "sheet.xlsx", "letter.docx", "archive.zip", "invite.ics"] {
            assert_eq!(verdict(name), Verdict::Open, "{name}");
        }
        assert!(Verdict::Macros.asks() && Verdict::Scripts.asks() && !Verdict::Open.asks() && !Verdict::Blocked.asks());
    }
}
