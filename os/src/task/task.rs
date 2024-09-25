//! Types related to task management

use super::TaskContext;
use crate::timer::get_time;

#[allow(dead_code)]
#[derive(Copy, Clone)]
pub struct TaskControlBlock {
    pub task_status: TaskStatus,
    pub task_cx: TaskContext,
    pub user_time: usize,
    pub kernel_time: usize,
}

#[allow(dead_code)]
impl TaskControlBlock {
    pub fn set_user_time(&mut self) {
        self.user_time = get_time()
    }
    pub fn get_user_time(&mut self) -> usize {
        get_time() - self.user_time
    }
    pub fn update_kernel_time(&mut self) -> usize {
        let last_time = self.kernel_time;
        self.kernel_time = get_time();
        self.kernel_time - last_time
    }
}

#[derive(Copy, Clone, PartialEq)]
pub enum TaskStatus {
    UnInit,
    Ready,
    Running,
    Exited,
}
