fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/quicktunnel.ico");

    #[cfg(target_os = "windows")]
    embed_windows_resources();
}

#[cfg(target_os = "windows")]
fn embed_windows_resources() {
    use std::path::Path;

    let ico_path = Path::new("assets/quicktunnel.ico");
    assert!(
        ico_path.is_file(),
        "missing Windows icon asset at {}",
        ico_path.display()
    );

    let mut res = winresource::WindowsResource::new();
    res.set_icon(ico_path.to_str().expect("non-UTF8 icon path"));
    res.set("ProductName", "QuickTunnel");
    res.set(
        "FileDescription",
        "QuickTunnel - Cloudflare Quick Tunnel Desktop App",
    );
    res.set("CompanyName", "QuickTunnel Contributors");
    res.set("OriginalFilename", "quicktunnel.exe");
    res.set("InternalName", "quicktunnel");
    res.set(
        "LegalCopyright",
        "Copyright (C) 2026 QuickTunnel Contributors. Licensed under GPL-3.0-or-later.",
    );
    res.set_manifest(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity
    version="0.1.0.0"
    processorArchitecture="*"
    name="QuickTunnel.DesktopApp"
    type="win32"
  />
  <description>QuickTunnel - Cloudflare Quick Tunnel Desktop App</description>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false" />
      </requestedPrivileges>
    </security>
  </trustInfo>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}" />
      <supportedOS Id="{1f676c76-80e1-4239-95bb-83d0f6d0da78}" />
      <supportedOS Id="{4a2f28e3-53b9-4441-ba9c-d69d4a4a6e38}" />
    </application>
  </compatibility>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true/pm</dpiAware>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2</dpiAwareness>
    </windowsSettings>
  </application>
</assembly>"#,
    );

    res.compile().expect("failed to compile Windows resources");
}
