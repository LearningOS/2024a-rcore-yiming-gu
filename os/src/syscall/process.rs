//! Process management syscalls
use crate::{
    config::MAX_SYSCALL_NUM,
    mm::{memory_set::MapPermission, translated_byte_buffer, VirtAddr},
    task::{
        change_program_brk, current_task_map_area, current_task_unmap_area, current_user_token, exit_current_and_run_next, suspend_current_and_run_next, TaskStatus,
        current_task_info,
    },
    timer::{get_time_ms, get_time_us},
};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// Task information
#[allow(dead_code)]
pub struct TaskInfo {
    /// Task status in it's life cycle
    status: TaskStatus,
    /// The numbers of syscall called by task
    syscall_times: [u32; MAX_SYSCALL_NUM],
    /// Total running time of task
    time: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(_exit_code: i32) -> ! {
    trace!("kernel: sys_exit");
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let us = get_time_us();
    let sec_bytes = (us / 1_000_000).to_ne_bytes();
    let usec_bytes = (us % 1_000_000).to_ne_bytes();
    let ts = _ts as *const u8;
    let mut ts_buf = translated_byte_buffer(current_user_token(), ts, 16);
    if ts_buf.len() == 1 {
        ts_buf[0][..8].copy_from_slice(&sec_bytes[..]);
        ts_buf[0][8..].copy_from_slice(&usec_bytes[..]);
    }

    0
}

/// YOUR JOB: Finish sys_task_info to pass testcases
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TaskInfo`] is splitted by two pages ?
pub fn sys_task_info(_ti: *mut TaskInfo) -> isize {
    trace!("kernel: sys_task_info NOT IMPLEMENTED YET!");
    let (syscall_times, stime) = current_task_info();
    let run_time = get_time_ms() - stime;

    let task_info = TaskInfo {
        status: TaskStatus::Running,
        syscall_times: syscall_times,
        time: run_time,
    };

    let ti_len = core::mem::size_of::<TaskInfo>();
    let mut ti_buf = translated_byte_buffer(current_user_token(), _ti as *const u8, 2016);

    if ti_buf.len() == 1 {
        unsafe {
            core::ptr::copy(
                &task_info as *const TaskInfo as *const u8,
                ti_buf[0].as_mut_ptr(),
                ti_len,
            );
        }
    }
    0
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    trace!("kernel: sys_mmap NOT IMPLEMENTED YET!");
    let start_va = VirtAddr::from(_start);
    let end_va = VirtAddr::from(_start + _len);
    if _port & !0x7 != 0 || _port & 0x7 == 0 || start_va.page_offset() != 0 {
        -1
    }
    else {
        let mut map_permission = MapPermission::U;
        if _port & 0x1 != 0 {
            map_permission |= MapPermission::R;
        }
        if _port & 0x2 != 0 {
            map_permission |= MapPermission::W;
        }
        if _port & 0x4 != 0 {
            map_permission |= MapPermission::X;
        }

        let result = current_task_map_area(start_va, end_va, map_permission);
        result
    }
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    trace!("kernel: sys_munmap NOT IMPLEMENTED YET!");
    let start_va = VirtAddr::from(_start);
    let end_va = VirtAddr::from(_start + _len);
    current_task_unmap_area(start_va, end_va)
}

/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel: sys_sbrk");
    if let Some(old_brk) = change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}
