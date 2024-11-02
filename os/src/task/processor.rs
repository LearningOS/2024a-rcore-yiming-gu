//!Implementation of [`Processor`] and Intersection of control flow
//!
//! Here, the continuous operation of user apps in CPU is maintained,
//! the current running state of CPU is recorded,
//! and the replacement and transfer of control flow of different applications are executed.

use super::__switch;
use super::{fetch_task, TaskStatus};
use super::{TaskContext, TaskControlBlock};
use crate::sync::UPSafeCell;
use crate::timer::get_time_ms;
use crate::trap::TrapContext;
use crate::syscall::TaskInfo;
use crate::mm::{MapPermission, VPNRange, VirtAddr};
use alloc::sync::Arc;
use lazy_static::*;

/// Processor management structure
pub struct Processor {
    ///The task currently executing on the current processor
    current: Option<Arc<TaskControlBlock>>,

    ///The basic control flow of each core, helping to select and switch process
    idle_task_cx: TaskContext,
}

impl Processor {
    ///Create an empty Processor
    pub fn new() -> Self {
        Self {
            current: None,
            idle_task_cx: TaskContext::zero_init(),
        }
    }

    ///Get mutable reference to `idle_task_cx`
    fn get_idle_task_cx_ptr(&mut self) -> *mut TaskContext {
        &mut self.idle_task_cx as *mut _
    }

    ///Get current task in moving semanteme
    pub fn take_current(&mut self) -> Option<Arc<TaskControlBlock>> {
        self.current.take()
    }

    ///Get current task in cloning semanteme
    pub fn current(&self) -> Option<Arc<TaskControlBlock>> {
        self.current.as_ref().map(Arc::clone)
    }
}

lazy_static! {
    pub static ref PROCESSOR: UPSafeCell<Processor> = unsafe { UPSafeCell::new(Processor::new()) };
}

///The main part of process execution and scheduling
///Loop `fetch_task` to get the process that needs to run, and switch the process through `__switch`
pub fn run_tasks() {
    loop {
        let mut processor = PROCESSOR.exclusive_access();
        if let Some(task) = fetch_task() {
            let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
            // access coming task TCB exclusively
            let mut task_inner = task.inner_exclusive_access();
            let next_task_cx_ptr = &task_inner.task_cx as *const TaskContext;
            task_inner.task_status = TaskStatus::Running;
            if task_inner.task_stime == 0 {
                task_inner.task_stime = get_time_ms();
            }
            // release coming task_inner manually
            drop(task_inner);
            // release coming task TCB manually
            processor.current = Some(task);
            // release processor manually
            drop(processor);
            unsafe {
                __switch(idle_task_cx_ptr, next_task_cx_ptr);
            }
        } else {
            warn!("no tasks available in run_tasks");
        }
    }
}

/// Get current task through take, leaving a None in its place
pub fn take_current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().take_current()
}

/// Get a copy of the current task
pub fn current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().current()
}

/// Get the current user token(addr of page table)
pub fn current_user_token() -> usize {
    let task = current_task().unwrap();
    task.get_user_token()
}

///Get the mutable reference to trap context of current task
pub fn current_trap_cx() -> &'static mut TrapContext {
    current_task()
        .unwrap()
        .inner_exclusive_access()
        .get_trap_cx()
}

///Record the number of syscalls of the current task
pub fn current_task_count_syscall(syscall_id: usize) {
    let processor = PROCESSOR.exclusive_access();
    let task = processor.current.as_ref().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    task_inner.task_syscall_times[syscall_id] += 1;
}

///Get the information of current task
pub fn current_task_info() -> TaskInfo {
    let task = current_task().unwrap();
    let task_inner = task.inner_exclusive_access();
    let current_time = get_time_ms();
    let running_time = current_time - task_inner.task_stime;

    let task_info = TaskInfo {
        status: task_inner.task_status,
        syscall_times: task_inner.task_syscall_times,
        time: running_time,
    };
    task_info
}

///Return to idle control flow for new scheduling
pub fn schedule(switched_task_cx_ptr: *mut TaskContext) {
    let mut processor = PROCESSOR.exclusive_access();
    let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
    drop(processor);
    unsafe {
        __switch(switched_task_cx_ptr, idle_task_cx_ptr);
    }
}

///current task map memory
pub fn current_task_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    let start_va = VirtAddr::from(_start);
    let end_va = VirtAddr::from(_start + _len);
    if start_va.page_offset() != 0 || _port & !0x7 != 0 || _port & 0x7 == 0{
        return -1;
    }
    let start_vpn = start_va.floor();
    let end_vpn = end_va.ceil();

    let processor = PROCESSOR.exclusive_access();
    let task = processor.current.as_ref().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    let memory_set = &mut task_inner.memory_set;

    let vpn_range = VPNRange {
        l: start_vpn,
        r: end_vpn,
    };

    for vpn in vpn_range {
        if let Some(vpn_map) = memory_set.translate(vpn) {
            if vpn_map.is_valid() {
                return -1;
            }
        }
    }

    let mut map_permission = MapPermission::U;
    map_permission |= MapPermission::from_bits((_port << 1) as u8).unwrap();

    memory_set.insert_framed_area(start_va, end_va, map_permission);

    0
}

///current task unmap memory
pub fn current_task_munmap(_start: usize, _len: usize) -> isize {
    let start_va = VirtAddr::from(_start);
    let end_va = VirtAddr::from(_start + _len);
    if start_va.page_offset() != 0 {
        return -1;
    }
    let start_vpn = start_va.floor();
    let end_vpn = end_va.ceil();

    let processor = PROCESSOR.exclusive_access();
    let task = processor.current.as_ref().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    let memory_set = &mut task_inner.memory_set;

    let vpn_range = VPNRange {
        l: start_vpn,
        r: end_vpn,
    };

    for vpn in vpn_range {
        let pte = memory_set.translate(vpn);
        if pte.is_none() {
            return -1;
        }
        else {
            if let Some(vpn_map) = pte {
                if !vpn_map.is_valid() {
                    return -1;
                }
            }
        }
    }

    memory_set.remove_area_with_start_vpn(start_vpn);

    0
}
