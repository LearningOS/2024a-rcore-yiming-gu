//! Process management syscalls
use crate::{
    config::MAX_SYSCALL_NUM,
    mm::{memory_set::MapPermission, translated_byte_buffer},
    task::{
        change_program_brk, current_task_map_area, current_user_token, exit_current_and_run_next, suspend_current_and_run_next, TaskStatus
    },
    timer::get_time_us
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
    let tus = (_ts as usize + 8) as *const u8;
    let mut sec= translated_byte_buffer(current_user_token(), ts, 8);
    let mut usec = translated_byte_buffer(current_user_token(), tus, 8);

    sec[0].copy_from_slice(&sec_bytes[..]);
    usec[0].copy_from_slice(&usec_bytes[..]);

    0
}

/// YOUR JOB: Finish sys_task_info to pass testcases
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TaskInfo`] is splitted by two pages ?
pub fn sys_task_info(_ti: *mut TaskInfo) -> isize {
    trace!("kernel: sys_task_info NOT IMPLEMENTED YET!");
    -1
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    trace!("kernel: sys_mmap NOT IMPLEMENTED YET!");
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
    current_task_map_area(_start, _start+_len, map_permission);
    0
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    trace!("kernel: sys_munmap NOT IMPLEMENTED YET!");
    0
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
