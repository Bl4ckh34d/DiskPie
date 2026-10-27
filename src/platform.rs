#[derive(Clone)]
pub struct Drive {
    pub path: String,
    pub total: u64,
    pub free: u64,
}
pub fn drives() -> Vec<Drive> {
    #[cfg(windows)]
    {
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn GetLogicalDrives() -> u32;
            fn GetDiskFreeSpaceExW(
                path: *const u16,
                available: *mut u64,
                total: *mut u64,
                free: *mut u64,
            ) -> i32;
            fn GetDriveTypeW(path: *const u16) -> u32;
        }
        let mut drives = Vec::new();
        // Fixed/removable local disks only: do not wake disconnected network shares.
        let mask = unsafe { GetLogicalDrives() };
        for i in 0..26 {
            if mask & (1 << i) != 0 {
                let path = format!("{}:\\", (b'A' + i) as char);
                let wide: Vec<_> = path.encode_utf16().chain([0]).collect();
                let kind = unsafe { GetDriveTypeW(wide.as_ptr()) };
                if kind != 2 && kind != 3 {
                    continue;
                }
                let (mut available, mut total, mut free) = (0, 0, 0);
                if unsafe {
                    GetDiskFreeSpaceExW(wide.as_ptr(), &mut available, &mut total, &mut free)
                } != 0
                {
                    drives.push(Drive {
                        path,
                        total,
                        free: available,
                    });
                }
            }
        }
        drives
    }
    #[cfg(not(windows))]
    {
        vec![Drive {
            path: "/".into(),
            total: 0,
            free: 0,
        }]
    }
}
