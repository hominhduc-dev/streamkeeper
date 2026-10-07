use anyhow::Result;

#[derive(Default)]
pub struct SleepInhibitor {
    #[cfg(windows)]
    handle: Option<std::os::windows::io::OwnedHandle>,
    pub error: Option<String>,
}
impl SleepInhibitor {
    pub fn active(&self) -> bool {
        #[cfg(windows)]
        {
            self.handle.is_some()
        }
        #[cfg(not(windows))]
        {
            false
        }
    }
    pub fn sync(&mut self, required: bool) -> Result<()> {
        #[cfg(windows)]
        {
            use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
            use windows_sys::Win32::{
                Foundation::INVALID_HANDLE_VALUE,
                System::{
                    Power::{
                        PowerClearRequest, PowerCreateRequest, PowerRequestSystemRequired,
                        PowerSetRequest,
                    },
                    Threading::{
                        POWER_REQUEST_CONTEXT_SIMPLE_STRING, REASON_CONTEXT, REASON_CONTEXT_0,
                    },
                },
            };
            if required && self.handle.is_none() {
                let mut reason = "Streamkeeper đang tải và xử lý video"
                    .encode_utf16()
                    .chain(Some(0))
                    .collect::<Vec<_>>();
                let context = REASON_CONTEXT {
                    Version: 0,
                    Flags: POWER_REQUEST_CONTEXT_SIMPLE_STRING,
                    Reason: REASON_CONTEXT_0 {
                        SimpleReasonString: reason.as_mut_ptr(),
                    },
                };
                // Windows copies the reason during this call; the UTF-16 buffer lives until it returns.
                let raw = unsafe { PowerCreateRequest(&context) };
                anyhow::ensure!(
                    !raw.is_null() && raw != INVALID_HANDLE_VALUE,
                    "Không tạo được yêu cầu chống sleep: {}",
                    std::io::Error::last_os_error()
                );
                // A valid, uniquely owned kernel handle; OwnedHandle closes it on drop, across Tokio threads.
                let handle = unsafe { OwnedHandle::from_raw_handle(raw) };
                anyhow::ensure!(
                    unsafe { PowerSetRequest(handle.as_raw_handle(), PowerRequestSystemRequired) }
                        != 0,
                    "Windows không chấp nhận yêu cầu chống sleep: {}",
                    std::io::Error::last_os_error()
                );
                self.handle = Some(handle);
            } else if !required {
                if let Some(handle) = self.handle.take() {
                    // Closing also releases the object's request if clearing fails.
                    unsafe {
                        PowerClearRequest(handle.as_raw_handle(), PowerRequestSystemRequired);
                    }
                }
            }
        }
        #[cfg(not(windows))]
        {
            anyhow::ensure!(!required, "Chống sleep hiện chỉ hỗ trợ Windows");
        }
        self.error = None;
        Ok(())
    }
}
impl Drop for SleepInhibitor {
    fn drop(&mut self) {
        let _ = self.sync(false);
    }
}

pub fn needs_awake(
    enabled: bool,
    closing: bool,
    states: impl IntoIterator<Item = impl AsRef<str>>,
) -> bool {
    enabled
        && !closing
        && states
            .into_iter()
            .any(|s| matches!(s.as_ref(), "downloading" | "muxing" | "verifying"))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lifecycle() {
        for state in ["downloading", "muxing", "verifying"] {
            assert!(needs_awake(true, false, [state]));
        }
        for state in ["queued", "paused", "failed", "cancelled", "completed"] {
            assert!(!needs_awake(true, false, [state]));
        }
        assert!(!needs_awake(false, false, ["downloading"]));
        assert!(!needs_awake(true, true, ["muxing"]));
    }
    #[cfg(windows)]
    #[test]
    fn windows_request_acquires_and_releases() {
        let mut power = SleepInhibitor::default();
        power.sync(true).unwrap();
        assert!(power.active());
        power.sync(true).unwrap();
        power.sync(false).unwrap();
        assert!(!power.active());
    }
}
