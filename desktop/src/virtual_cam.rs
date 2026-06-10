use anyhow::{Context, Result};
use bytes::Bytes;
use image::load_from_memory;
use std::sync::Arc;
use tokio::sync::broadcast;
use windows::core::PCSTR;
use windows::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE, WAIT_OBJECT_0};
use windows::Win32::System::Memory::{
    CreateFileMappingA, MapViewOfFile, FILE_MAP_ALL_ACCESS, PAGE_READWRITE,
};
use windows::Win32::System::Threading::{
    CreateEventA, CreateMutexA, ReleaseMutex, SetEvent, WaitForSingleObject,
};

pub struct AppState {
    pub frame_rx: broadcast::Sender<Bytes>,
}

#[repr(C)]
struct SharedImageMemoryHeader {
    magic: u32,
    width: u32,
    height: u32,
    stride: u32,
    format: u32,
    resize_mode: u32,
    mirror_mode: u32,
    timeout: u32,
    frame_num: u32,
    frame_tick: u32,
}

pub async fn run_virtual_camera(state: Arc<AppState>) -> Result<()> {
    let mut rx = state.frame_rx.subscribe();

    tokio::task::spawn_blocking(move || -> Result<()> {
        let max_shared_image_size = 3840 * 2160 * 4 * 2;
        let header_size = std::mem::size_of::<SharedImageMemoryHeader>() as u32;
        let total_size = header_size + max_shared_image_size;

        unsafe {
            let mem_name = PCSTR::from_raw("CutieCam_Memory_0\0".as_ptr());
            let mutex_name = PCSTR::from_raw("CutieCam_Mutex_0\0".as_ptr());
            let event_name = PCSTR::from_raw("CutieCam_Event_0\0".as_ptr());

            let h_map =
                CreateFileMappingA(INVALID_HANDLE_VALUE, None, PAGE_READWRITE, 0, total_size, mem_name)
                    .context("Failed to create file mapping")?;
            let h_mutex = CreateMutexA(None, false, mutex_name).context("Failed to create mutex")?;
            let h_event =
                CreateEventA(None, false, false, event_name).context("Failed to create event")?;

            let p_buf = MapViewOfFile(h_map, FILE_MAP_ALL_ACCESS, 0, 0, total_size as usize);
            if p_buf.Value.is_null() {
                anyhow::bail!("Failed to map view of file");
            }

            let p_header = p_buf.Value as *mut SharedImageMemoryHeader;
            let p_pixels = (p_buf.Value as *mut u8).add(header_size as usize);

            let mut frame_num: u32 = 0;

            println!("CutieCam Windows Driver active! Streaming in the background...");

            loop {
                let frame_bytes = match rx.blocking_recv() {
                    Ok(b) => b,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                };

                let img = match load_from_memory(&frame_bytes) {
                    Ok(img) => img.into_rgba8(),
                    Err(_) => continue,
                };

                let width = img.width();
                let height = img.height();
                let raw_pixels = img.as_raw();
                let frame_size = raw_pixels.len();

                if WaitForSingleObject(h_mutex, 100) == WAIT_OBJECT_0 {
                    (*p_header).magic = 0x8256E101;
                    (*p_header).width = width;
                    (*p_header).height = height;
                    (*p_header).stride = width;
                    (*p_header).format = 0;
                    (*p_header).resize_mode = 1;
                    (*p_header).mirror_mode = 0;
                    (*p_header).timeout = 5000;

                    frame_num = frame_num.wrapping_add(1);
                    (*p_header).frame_num = frame_num;

                    std::ptr::copy_nonoverlapping(raw_pixels.as_ptr(), p_pixels, frame_size);

                    let _ = ReleaseMutex(h_mutex);
                    let _ = SetEvent(h_event);
                }
            }

            let _ = CloseHandle(h_map);
            let _ = CloseHandle(h_mutex);
            let _ = CloseHandle(h_event);
        }

        Ok(())
    })
        .await??;

    Ok(())
}