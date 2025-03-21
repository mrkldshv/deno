// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::cache::DiskCache;

use std::path::PathBuf;

/// `DenoDir` serves as coordinator for multiple `DiskCache`s containing them
/// in single directory that can be controlled with `$DENO_DIR` env variable.
#[derive(Clone)]
pub struct DenoDir {
  /// Example: /Users/rld/.deno/
  pub root: PathBuf,
  /// Used by TsCompiler to cache compiler output.
  pub gen_cache: DiskCache,
}

impl DenoDir {
  pub fn new(maybe_custom_root: Option<PathBuf>) -> std::io::Result<Self> {
    let root: PathBuf = if let Some(root) = maybe_custom_root {
      root
    } else if let Some(cache_dir) = dirs::cache_dir() {
      // We use the OS cache dir because all files deno writes are cache files
      // Once that changes we need to start using different roots if DENO_DIR
      // is not set, and keep a single one if it is.
      cache_dir.join("deno")
    } else if let Some(home_dir) = dirs::home_dir() {
      // fallback path
      home_dir.join(".deno")
    } else {
      panic!("Could not set the Deno root directory")
    };
    let root = if root.is_absolute() {
      root
    } else {
      std::env::current_dir()?.join(root)
    };
    assert!(root.is_absolute());
    let gen_path = root.join("gen");

    let deno_dir = Self {
      root,
      gen_cache: DiskCache::new(&gen_path),
    };
    deno_dir.gen_cache.ensure_dir_exists(&gen_path)?;

    Ok(deno_dir)
  }

  /// Path for the incremental cache used for formatting.
  pub fn fmt_incremental_cache_db_file_path(&self) -> PathBuf {
    // bump this version name to invalidate the entire cache
    self.root.join("fmt_incremental_cache_v1")
  }

  /// Path for the incremental cache used for linting.
  pub fn lint_incremental_cache_db_file_path(&self) -> PathBuf {
    // bump this version name to invalidate the entire cache
    self.root.join("lint_incremental_cache_v1")
  }

  /// Path for the incremental cache used for linting.
  pub fn type_checking_cache_db_file_path(&self) -> PathBuf {
    // bump this version name to invalidate the entire cache
    self.root.join("check_cache_v1")
  }
}

/// To avoid the poorly managed dirs crate
#[cfg(not(windows))]
mod dirs {
  use std::path::PathBuf;

  pub fn cache_dir() -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
      home_dir().map(|h| h.join("Library/Caches"))
    } else {
      std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| home_dir().map(|h| h.join(".cache")))
    }
  }

  pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
      .and_then(|h| if h.is_empty() { None } else { Some(h) })
      .or_else(|| {
        // TODO(bartlomieju):
        #[allow(clippy::undocumented_unsafe_blocks)]
        unsafe {
          fallback()
        }
      })
      .map(PathBuf::from)
  }

  // This piece of code is taken from the deprecated home_dir() function in Rust's standard library: https://github.com/rust-lang/rust/blob/master/src/libstd/sys/unix/os.rs#L579
  // The same code is used by the dirs crate
  unsafe fn fallback() -> Option<std::ffi::OsString> {
    let amt = match libc::sysconf(libc::_SC_GETPW_R_SIZE_MAX) {
      n if n < 0 => 512_usize,
      n => n as usize,
    };
    let mut buf = Vec::with_capacity(amt);
    let mut passwd: libc::passwd = std::mem::zeroed();
    let mut result = std::ptr::null_mut();
    match libc::getpwuid_r(
      libc::getuid(),
      &mut passwd,
      buf.as_mut_ptr(),
      buf.capacity(),
      &mut result,
    ) {
      0 if !result.is_null() => {
        let ptr = passwd.pw_dir as *const _;
        let bytes = std::ffi::CStr::from_ptr(ptr).to_bytes().to_vec();
        Some(std::os::unix::ffi::OsStringExt::from_vec(bytes))
      }
      _ => None,
    }
  }
}

/// To avoid the poorly managed dirs crate
// Copied from
// https://github.com/dirs-dev/dirs-sys-rs/blob/ec7cee0b3e8685573d847f0a0f60aae3d9e07fa2/src/lib.rs#L140-L164
// MIT license. Copyright (c) 2018-2019 dirs-rs contributors
#[cfg(windows)]
mod dirs {
  use std::ffi::OsString;
  use std::os::windows::ffi::OsStringExt;
  use std::path::PathBuf;
  use winapi::shared::winerror;
  use winapi::um::{combaseapi, knownfolders, shlobj, shtypes, winbase, winnt};

  fn known_folder(folder_id: shtypes::REFKNOWNFOLDERID) -> Option<PathBuf> {
    // SAFETY: winapi calls
    unsafe {
      let mut path_ptr: winnt::PWSTR = std::ptr::null_mut();
      let result = shlobj::SHGetKnownFolderPath(
        folder_id,
        0,
        std::ptr::null_mut(),
        &mut path_ptr,
      );
      if result == winerror::S_OK {
        let len = winbase::lstrlenW(path_ptr) as usize;
        let path = std::slice::from_raw_parts(path_ptr, len);
        let ostr: OsString = OsStringExt::from_wide(path);
        combaseapi::CoTaskMemFree(path_ptr as *mut winapi::ctypes::c_void);
        Some(PathBuf::from(ostr))
      } else {
        None
      }
    }
  }

  pub fn cache_dir() -> Option<PathBuf> {
    known_folder(&knownfolders::FOLDERID_LocalAppData)
  }

  pub fn home_dir() -> Option<PathBuf> {
    known_folder(&knownfolders::FOLDERID_Profile)
  }
}
