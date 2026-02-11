use leodos_protocols::datalink::DataLink;
use leodos_protocols::network::isl::routing::local::LocalChannel;
use leodos_protocols::network::NetworkLayer;

#[test]
fn local_channel_app_to_router() {
    futures::executor::block_on(async {
        let channel: LocalChannel<8, 256> = LocalChannel::new();
        let (mut app, mut router) = channel.split();

        let test_data = b"Hello from app to router!";
        app.send(test_data).await.unwrap();

        let mut buffer = [0u8; 256];
        let len = router.recv(&mut buffer).await.unwrap();

        assert_eq!(&buffer[..len], test_data);
    });
}

#[test]
fn local_channel_router_to_app() {
    futures::executor::block_on(async {
        let channel: LocalChannel<8, 256> = LocalChannel::new();
        let (mut app, mut router) = channel.split();

        let test_data = b"Hello from router to app!";
        router.send(test_data).await.unwrap();

        let mut buffer = [0u8; 256];
        let len = app.recv(&mut buffer).await.unwrap();

        assert_eq!(&buffer[..len], test_data);
    });
}

#[test]
fn local_channel_bidirectional() {
    futures::executor::block_on(async {
        let channel: LocalChannel<8, 256> = LocalChannel::new();
        let (mut app, mut router) = channel.split();

        let app_msg = b"Message from app";
        let router_msg = b"Message from router";

        app.send(app_msg).await.unwrap();
        router.send(router_msg).await.unwrap();

        let mut buffer = [0u8; 256];

        let len = router.recv(&mut buffer).await.unwrap();
        assert_eq!(&buffer[..len], app_msg);

        let len = app.recv(&mut buffer).await.unwrap();
        assert_eq!(&buffer[..len], router_msg);
    });
}

#[test]
fn local_channel_multiple_messages() {
    futures::executor::block_on(async {
        let channel: LocalChannel<8, 256> = LocalChannel::new();
        let (mut app, mut router) = channel.split();

        for i in 0..5 {
            let msg = format!("Message {}", i);
            app.send(msg.as_bytes()).await.unwrap();
        }

        let mut buffer = [0u8; 256];
        for i in 0..5 {
            let len = router.recv(&mut buffer).await.unwrap();
            let expected = format!("Message {}", i);
            assert_eq!(&buffer[..len], expected.as_bytes());
        }
    });
}
