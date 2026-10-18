//! Windows only: embed the app icon and version info (VERSIONINFO) into `mailcraft.exe`, so it
//! shows in Explorer, the taskbar, the Start menu and Alt-Tab.
//!
//! On every other target this does nothing. A missing resource compiler is a warning, so a
//! cross-compile from macOS or Linux still links, unless `MAILCRAFT_REQUIRE_WINRES=1` turns it
//! into an error (for release builds).

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../assets/branding/despres-connect/mailcraft.ico");
    println!("cargo:rerun-if-env-changed=MAILCRAFT_REQUIRE_WINRES");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("../../assets/branding/despres-connect/mailcraft.ico")
        .set("ProductName", "MailCraft")
        .set("FileDescription", "MailCraft mail and calendar")
        .set("CompanyName", "MailCraft contributors")
        .set("LegalCopyright", "Copyright (c) the MailCraft authors. MIT OR Apache-2.0.")
        .set("OriginalFilename", "mailcraft.exe")
        .set("InternalName", "mailcraft");
    if let Err(e) = res.compile() {
        if std::env::var_os("MAILCRAFT_REQUIRE_WINRES").is_some() {
            eprintln!("embedding Windows resources failed: {e}");
            std::process::exit(1);
        }
        println!("cargo:warning=mailcraft.exe built without icon/version resources: {e}");
    }
}
