use crate::buffer::{Buffer, SharedBuffer};
use parking_lot::{Condvar, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

/// 异步循环器，用于后台日志处理
///
/// 运行一个后台线程，从推送缓冲区处理日志消息并调用回调函数。
pub struct AsyncLooper {
    running: Arc<AtomicBool>,
    #[allow(dead_code)]
    push_cond: Arc<Condvar>,
    pop_cond: Arc<Condvar>,
    #[allow(dead_code)]
    mutex: Arc<Mutex<()>>,
    tasks_push: SharedBuffer,
    #[allow(dead_code)]
    tasks_pop: SharedBuffer,
    handle: Option<thread::JoinHandle<()>>,
}

impl AsyncLooper {
    /// 创建新的异步循环器
    pub fn new<F>(callback: F) -> Self
    where
        F: Fn(&mut Buffer) + Send + Sync + 'static,
    {
        let running = Arc::new(AtomicBool::new(true));
        let mutex = Arc::new(Mutex::new(()));
        let push_cond = Arc::new(Condvar::new());
        let pop_cond = Arc::new(Condvar::new());
        let tasks_push = SharedBuffer::new();
        let tasks_pop = SharedBuffer::new();

        // 启动后台线程
        let running_clone = Arc::clone(&running);
        let mutex_clone = Arc::clone(&mutex);
        let push_cond_clone = Arc::clone(&push_cond);
        let pop_cond_clone = Arc::clone(&pop_cond);
        let tasks_push_clone = tasks_push.clone();
        let tasks_pop_clone = tasks_pop.clone();

        let handle = thread::Builder::new()
            .name("bitlog-async".to_string())
            .spawn(move || {
                Self::worker_loop(
                    &running_clone,
                    &push_cond_clone,
                    &pop_cond_clone,
                    &mutex_clone,
                    &tasks_push_clone,
                    &tasks_pop_clone,
                    callback,
                );
            })
            .expect("Failed to start async looper thread");

        AsyncLooper {
            running,
            push_cond,
            pop_cond,
            mutex,
            tasks_push,
            tasks_pop,
            handle: Some(handle),
        }
    }

    /// 推送消息到队列
    pub fn push(&self, msg: &str) {
        if !self.running.load(Ordering::Relaxed) {
            return;
        }

        // 等待缓冲区有足够空间
        while self.tasks_push.readable_size() > 100_000_000 {
            thread::sleep(std::time::Duration::from_millis(1));
            if !self.running.load(Ordering::Relaxed) {
                return;
            }
        }

        self.tasks_push.push_str(msg);
        self.pop_cond.notify_all();
    }

    /// 停止循环器
    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
        self.pop_cond.notify_all();

        if let Some(handle) = &self.handle {
            handle.thread().unpark();
        }
    }

    // 工作线程循环
    fn worker_loop<F>(
        running: &AtomicBool,
        push_cond: &Condvar,
        pop_cond: &Condvar,
        mutex: &Mutex<()>,
        tasks_push: &SharedBuffer,
        tasks_pop: &SharedBuffer,
        callback: F,
    ) where
        F: Fn(&mut Buffer) + Send + Sync,
    {
        loop {
            {
                let mut guard = mutex.lock();

                // 检查是否已停止且缓冲区为空
                if !running.load(Ordering::Relaxed) && tasks_push.is_empty() {
                    return;
                }

                // 等待数据或停止信号
                pop_cond.wait_while(&mut guard, |_| {
                    tasks_push.is_empty() && running.load(Ordering::Relaxed)
                });

                // 交换推送和弹出缓冲区
                tasks_push.swap_with(tasks_pop);
            }

            // 通知生产者空间可用
            push_cond.notify_all();

            // 处理弹出的数据
            tasks_pop.with_lock(|buf| {
                if !buf.is_empty() {
                    callback(buf);
                }
                buf.reset();
            });
        }
    }
}

impl Drop for AsyncLooper {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    // use super::*;
    // use std::sync::atomic::{AtomicUsize, Ordering};

    // #[test]
    // fn test_async_looper() {
    //     let count = Arc::new(AtomicUsize::new(0));
    //     let count_clone = Arc::clone(&count);

    //     let looper = AsyncLooper::new(move |buf| {
    //         let data = buf.to_string();
    //         if !data.is_empty() {
    //             count_clone.fetch_add(1, Ordering::Relaxed);
    //         }
    //     });

    //     looper.push("test message 1");
    //     looper.push("test message 2");

    //     // 给后台线程时间处理消息
    //     thread::sleep(std::time::Duration::from_millis(200));

    //     looper.stop();

    //     // 等待线程完全停止
    //     thread::sleep(std::time::Duration::from_millis(50));

    //     assert!(count.load(Ordering::Relaxed) >= 2);
    // }
}
