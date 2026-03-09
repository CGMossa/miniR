# winresource integration plan

> `winresource` 0.1 — Embed Windows resources (icons, version info) in executables.
> <https://github.com/nicklasmoeller/winresource>

## What it does

Build-time tool that compiles Windows resource files (.rc) and links them into
the executable. Sets application icon, version info, and other Windows metadata.

```rust
// build.rs
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows" {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/newr.ico");
        res.set("ProductName", "newr");
        res.set("FileDescription", "R Interpreter");
        res.compile().unwrap();
    }
}
```

## Where it fits in newr

### 1. Windows executable branding

Sets the icon and version info shown in Windows Explorer, Task Manager, etc.
Pure cosmetic but professional.

## Recommendation

**Add when preparing Windows release builds.** Build-only dependency,
Windows-only, zero runtime impact.

**Effort:** 10 minutes.
