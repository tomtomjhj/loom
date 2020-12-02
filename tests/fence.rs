#![deny(warnings, rust_2018_idioms)]

use loom::cell::UnsafeCell;
use loom::sync::atomic::{fence, AtomicBool, AtomicUsize};
use loom::thread;

use std::sync::atomic::Ordering::{Acquire, Relaxed, Release, SeqCst};
use std::sync::Arc;

#[test]
fn fence_sw_base() {
    loom::model(|| {
        let data = Arc::new(UnsafeCell::new(0));
        let flag = Arc::new(AtomicBool::new(false));

        let th = {
            let (data, flag) = (data.clone(), flag.clone());
            thread::spawn(move || {
                data.with_mut(|ptr| unsafe { *ptr = 42 });
                fence(Release);
                flag.store(true, Relaxed);
            })
        };

        if flag.load(Relaxed) {
            fence(Acquire);
            assert_eq!(42, data.with_mut(|ptr| unsafe { *ptr }));
        }
        th.join().unwrap();
    });
}

#[test]
fn fence_sw_collapsed_store() {
    loom::model(|| {
        let data = Arc::new(UnsafeCell::new(0));
        let flag = Arc::new(AtomicBool::new(false));

        let th = {
            let (data, flag) = (data.clone(), flag.clone());
            thread::spawn(move || {
                data.with_mut(|ptr| unsafe { *ptr = 42 });
                flag.store(true, Release);
            })
        };

        if flag.load(Relaxed) {
            fence(Acquire);
            assert_eq!(42, data.with_mut(|ptr| unsafe { *ptr }));
        }
        th.join().unwrap();
    });
}

#[test]
fn fence_sw_collapsed_load() {
    loom::model(|| {
        let data = Arc::new(UnsafeCell::new(0));
        let flag = Arc::new(AtomicBool::new(false));

        let th = {
            let (data, flag) = (data.clone(), flag.clone());
            thread::spawn(move || {
                data.with_mut(|ptr| unsafe { *ptr = 42 });
                fence(Release);
                flag.store(true, Relaxed);
            })
        };

        if flag.load(Acquire) {
            assert_eq!(42, data.with_mut(|ptr| unsafe { *ptr }));
        }
        th.join().unwrap();
    });
}

#[test]
#[should_panic]
/// genmc/tests/wrong/racy/MPU+relf+rlx/mpu+relf+rlx.c
fn mpu_relf_rlx() {
    loom::model(|| {
        let x = Arc::new(UnsafeCell::new(0));
        let y = Arc::new(AtomicUsize::new(0));

        let th1 = {
            let (x, y) = (x.clone(), y.clone());
            thread::spawn(move || {
                x.with_mut(|ptr| unsafe { *ptr = 42 });
                fence(Release);
                y.store(1, Relaxed);
            })
        };
        let th2 = {
            let y = y.clone();
            thread::spawn(move || {
                y.fetch_add(1, Relaxed);
            })
        };

        if y.load(Relaxed) > 1 {
            x.with_mut(|ptr| unsafe { *ptr = 43 });
        }
        th1.join().unwrap();
        th2.join().unwrap();
    });
}

#[test]
#[should_panic]
/// genmc/tests/wrong/racy/MPU2+relf+rlx/mpu2+relf+rlx.c
fn mpu2_relf_rlx() {
    loom::model(|| {
        let x = Arc::new(UnsafeCell::new(0));
        let y = Arc::new(AtomicUsize::new(0));

        let th1 = {
            let (x, y) = (x.clone(), y.clone());
            thread::spawn(move || {
                x.with_mut(|ptr| unsafe { *ptr = 42 });
                fence(Release);
                y.store(1, Relaxed);
            })
        };
        let th2 = {
            let y = y.clone();
            thread::spawn(move || {
                y.compare_and_swap(2, 3, Relaxed);
            })
        };
        let th3 = {
            let y = y.clone();
            thread::spawn(move || {
                y.compare_and_swap(1, 2, Relaxed);
            })
        };

        if y.load(Relaxed) > 1 {
            x.with_mut(|ptr| unsafe { *ptr = 43 });
        }
        th1.join().unwrap();
        th2.join().unwrap();
        th3.join().unwrap();
    });
}

#[test]
// SB+fences example from Promising
fn sb_fences() {
    loom::model(|| {
        let x = Arc::new(AtomicBool::new(false));
        let y = Arc::new(AtomicBool::new(false));

        let a = {
            let (x, y) = (x.clone(), y.clone());
            thread::spawn(move || {
                x.store(true, Relaxed);
                fence(SeqCst);
                y.load(Relaxed)
            })
        };

        y.store(true, Relaxed);
        fence(SeqCst);
        let b = x.load(Relaxed);

        if !a.join().unwrap() {
            assert!(b);
        }
    });
}

#[test]
fn fence_hazard_pointer() {
    loom::model(|| {
        let reachable = Arc::new(AtomicBool::new(true));
        let protected = Arc::new(AtomicBool::new(false));
        let allocated = Arc::new(AtomicBool::new(true));

        let th = {
            let (reachable, protected, allocated) =
                (reachable.clone(), protected.clone(), allocated.clone());
            thread::spawn(move || {
                // put in protected list
                protected.store(true, Relaxed);
                fence(SeqCst);
                // validate, then access
                if reachable.load(Relaxed) {
                    assert!(allocated.load(Relaxed));
                }
            })
        };

        // unlink/retire
        reachable.store(false, Relaxed);
        fence(SeqCst);
        // reclaim unprotected
        if !protected.load(Relaxed) {
            allocated.store(false, Relaxed);
        }

        th.join().unwrap();
    });
}
