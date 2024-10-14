//!Implementation of [`Processor`] and Intersection of control flow

use super::__switch;
use super::{fetch_task, TaskStatus};
use super::{TaskContext, TaskControlBlock};
use crate::sync::UPSafeCell;
use crate::trap::TrapContext;
use alloc::sync::Arc;
use lazy_static::*;

///Processor management structure
/// 使用processor，维护单个CPU上正在执行的进程
pub struct Processor {
    ///The task currently executing on the current processor
    /// 单个CPU上正在运行的进程，如果是None则该CPU空闲
    current: Option<Arc<TaskControlBlock>>,
    ///The basic control flow of each core, helping to select and switch process
    /// idle控制流
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
    /// 直接取出当前进程
    pub fn take_current(&mut self) -> Option<Arc<TaskControlBlock>> {
        self.current.take()
    }
    ///Get current task in cloning semanteme
    /// 产生一个当前进程的引用拷贝
    pub fn current(&self) -> Option<Arc<TaskControlBlock>> {
        self.current.as_ref().map(Arc::clone)
    }
}

lazy_static! {
    // 现在是单核状态，所以只定义了一个PROCESSOR
    pub static ref PROCESSOR: UPSafeCell<Processor> = unsafe { UPSafeCell::new(Processor::new()) };
}
///The main part of process execution and scheduling
///Loop `fetch_task` to get the process that needs to run, and switch the process through `__switch`
/// run_tasks是真正意义上的任务进程维护的主函数
/// 所有任务在执行完毕之后，都会返回到这个函数进行下一个任务的寻找工作
/// 他通过循环保证获取到一个可以执行的task
/// 然后从idle控制流切换到下一个应用程序控制流
/// 原先所谓的"run_next"功能都被集成到了run_task里，而不是分散在其他函数的末尾
pub fn run_tasks() {
    // 通过循环保证获取到下一个执行的task
    loop {
        let mut processor = PROCESSOR.exclusive_access();
        // 调用fetch_task从任务管理器里获取一个ready task
        if let Some(task) = fetch_task() {
            let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
            // access coming task TCB exclusively
            let mut task_inner = task.inner_exclusive_access();
            let next_task_cx_ptr = &task_inner.task_cx as *const TaskContext;
            task_inner.task_status = TaskStatus::Running;
            drop(task_inner);
            // release coming task TCB manually
            processor.current = Some(task);
            // release processor manually
            drop(processor);
            // println!("switch to next task");
            unsafe {
                __switch(idle_task_cx_ptr, next_task_cx_ptr);
            }
        }
    }
}
///Take the current task,leaving a None in its place
pub fn take_current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().take_current()
}
///Get running task
pub fn current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().current()
}
///Get token of the address space of current task
pub fn current_user_token() -> usize {
    let task = current_task().unwrap();
    let token = task.inner_exclusive_access().get_user_token();
    token
}
///Get the mutable reference to trap context of current task
pub fn current_trap_cx() -> &'static mut TrapContext {
    current_task()
        .unwrap()
        .inner_exclusive_access()
        .get_trap_cx()
}
///Return to idle control flow for new scheduling
/// 弹出当前task
/// 注意，会使用一个idle控制流进行替代
/// 这个idle控制流本身没有任何含义，他只是一个占位符，表示当前处于空闲状态
/// 当run_tasks调用switch进入下一个任务的时候，这个idle控制流就会被替换掉
pub fn schedule(switched_task_cx_ptr: *mut TaskContext) {
    let mut processor = PROCESSOR.exclusive_access();
    let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
    drop(processor);
    unsafe {
        __switch(switched_task_cx_ptr, idle_task_cx_ptr);
    }
}
