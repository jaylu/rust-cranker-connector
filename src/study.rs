
#[cfg(test)]
mod study {
    
    use super::*;
    use std::borrow::{Borrow, BorrowMut};
    use std::cell::{RefCell, RefMut};
    use std::collections::HashMap;
    use std::rc::Rc;
    use std::slice::RChunks;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;
    use tokio::sync::broadcast;
    use tokio::time;

    #[test]
    fn test_vec() {
        let sample = vec!["a", "b", "c"];
        let index = 2;
        println!("output: {:?}", &sample[0..index]);
    }

    #[test]
    fn test_vec_chunks() {
        let sample: Vec<&str> = vec!["a", "b", "c", "d", "e"];
        let collects: Vec<&[&str]> = sample.chunks(2).collect();
        for item in collects {
            println!("output: {:?}", item);
        }
    }

    #[test]
    fn test_option() {
        let mut v: Option<String> = None;
        println!("output: {:?}", &v);

        v.replace(String::from("Hello"));
        println!("output: {:?}", &v);
    }

    #[test]
    fn test_smart_pointer_arc() {
        //--nocapture
        let array = vec!["a", "b", "c"];
        let rc = Arc::new(array);

        let rc1 = Arc::clone(&rc);
        let rc2 = Arc::clone(&rc);

        let mut handles = vec![];

        let handle_1 = thread::spawn(move || println!("{:?}", rc1));
        handles.push(handle_1);

        let handle_2 = thread::spawn(move || println!("{:?}", rc2));
        handles.push(handle_2);

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_smart_pointer_arc_with_mutex() {
        //--nocapture
        let array = vec!["a", "b", "c"];
        let rc = Arc::new(Mutex::new(array));

        let rc1 = Arc::clone(&rc);
        let rc2 = Arc::clone(&rc);

        let mut handles = vec![];

        let handle_1 = thread::spawn(move || {
            println!("rc1: {:?}", rc1);
            rc1.lock().unwrap().push("d");
        });
        handles.push(handle_1);

        let handle_2 = thread::spawn(move || {
            println!("rc2: {:?}", rc2);
            rc2.lock().unwrap().push("e");
        });
        handles.push(handle_2);

        for handle in handles {
            handle.join().unwrap();
        }

        println!("rc: {:?}", rc);
    }

    #[test]
    fn test_smart_pointer_box() {
        //--nocapture
        let array = vec!["a", "b", "c"];
        let mut value = Box::new(array);

        println!("box before update: {:?}", &value);

        value.push("d");
        value.push("e");

        println!("box after update: {:?}", &value);
    }

    #[test]
    fn test_tokio_select() {
        //     tokio::select! {
        //     _ = async {
        //         while &isStarted {
        //             &interval.tick().await;
        //         }
        //         println!("complete")
        //     } => {},
        //     _cancel = rx => {
        //         isStarted = false;
        //         println!("cancel")
        //     }
        // }
    }

    #[tokio::test]
    async fn tokio_broadcast_test() {
        // one producer, multiple subscriber scenario
        let (tx, _) = broadcast::channel::<String>(16);
        let mut rx_1 = tx.subscribe();
        let mut rx_2 = tx.subscribe();

        let handle_1 = tokio::spawn(async move {
            assert_eq!(rx_1.recv().await.is_err(), true);
            println!("rx_1 done")
        });

        let handle_2 = tokio::spawn(async move {
            assert_eq!(rx_2.recv().await.is_err(), true);
            println!("rx_2 done")
        });

        drop(tx);
        let _ = handle_1.await;
        let _ = handle_2.await;
    }

    #[tokio::test]
    async fn tokio_broadcast_send_after_receiver_drop() {
        // one producer, multiple subscriber scenario
        let (tx, _) = broadcast::channel::<()>(16);
        let mut rx_1 = tx.subscribe();
        let is_break = Arc::new(AtomicBool::new(false));
        let is_break_clone = is_break.clone();

        let handle_1 = tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(5));
            loop {
                tokio::select! {
                    _ = interval.tick() => {}
                    rec = rx_1.recv() => {
                        assert_eq!(rec.is_err(), false);
                        break;
                    }
                }
                println!("rx_1 dropped")
            }

            is_break.store(true, Ordering::SeqCst);
        });

        assert_eq!(is_break_clone.load(Ordering::Acquire), false);
        let _ = tx.send(());
        let _ = handle_1.await;
        assert_eq!(is_break_clone.load(Ordering::Acquire), true);
        let result = tx.send(());
        assert_eq!(result.is_err(), true);
    }

    #[tokio::test]
    async fn tokio_broadcast_select_test_drop() {
        // one producer, multiple subscriber scenario
        let (tx, _) = broadcast::channel::<()>(16);
        let mut rx_1 = tx.subscribe();
        let is_break = Arc::new(AtomicBool::new(false));
        let is_break_clone = is_break.clone();

        let handle_1 = tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(5));
            loop {
                tokio::select! {
                    _ = interval.tick() => {}
                    rec = rx_1.recv() => {
                        assert_eq!(rec.is_err(), true);
                        break;
                    }
                }
            }

            is_break.store(true, Ordering::SeqCst);
        });

        assert_eq!(is_break_clone.load(Ordering::Acquire), false);
        drop(tx);
        let _ = handle_1.await;
        assert_eq!(is_break_clone.load(Ordering::Acquire), true);
    }

    #[tokio::test]
    async fn tokio_broadcast_select_test_send() {
        // one producer, multiple subscriber scenario
        let (tx, _) = broadcast::channel::<()>(16);
        let mut rx_1 = tx.subscribe();
        let is_break = Arc::new(AtomicBool::new(false));
        let is_break_clone = is_break.clone();

        let handle_1 = tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(5));
            loop {
                tokio::select! {
                    _ = interval.tick() => {}
                    rec = rx_1.recv() => {
                        assert_eq!(rec.is_err(), false);
                        break;
                    }
                }
            }

            is_break.store(true, Ordering::SeqCst);
        });

        assert_eq!(is_break_clone.load(Ordering::Acquire), false);
        let _ = tx.send(());
        let _ = handle_1.await;
        assert_eq!(is_break_clone.load(Ordering::Acquire), true);
    }
}