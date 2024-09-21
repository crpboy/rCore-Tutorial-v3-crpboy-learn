use core::{arch::asm, ptr::null};

/// print current stack info:
/// - return address
/// - file pointer
#[allow(dead_code)]
pub fn trace_stack() -> () {
    let mut fp: *const usize;
    let mut count: i32 = 0; // TODO: delete this
    unsafe {
        asm!("mv {}, fp", out(reg) fp);
        println!("\nStack tracing info:");
        println!("==== Begin stack trace ====");
        while fp != null() {
            count += 1;
            if count > 100 {
                println!("dead loop when tracing stack");
                return;
            }
            let cur_ra = *fp.sub(1);
            let last_fp = *fp.sub(2);
            println!("0x{:016x}, fp: 0x{:016x}", cur_ra as usize, fp as usize);
            fp = last_fp as *const usize;
        }
        println!("==== End stack trace ====\n");
    }
}

#[allow(dead_code)]
pub fn fetch_time() -> usize {
    let time: usize;
    unsafe {
        asm!("rdtime {}", out(reg) time);
    }
    time
}

/// for function timing statistic
#[macro_export]
macro_rules! statistic_time {
    ($func: expr) => {
        use crate::utils::fetch_time;
        let start_time = fetch_time();
        $func;
        let end_time = fetch_time();
        // let cost_value = ((end_time - start_time) as f32) / 1000f32;
        let cost_value = end_time - start_time;
        println!(
            "\x1b[32mtime: {} ticks\x1b[0m",
            cost_value
        );
    };
}
