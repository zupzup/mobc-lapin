use futures::StreamExt;
use lapin::{
    options::*, publisher_confirm::Confirmation, types::FieldTable, BasicProperties,
    ConnectionProperties,
};
use mobc::Pool;
use mobc_lapin::RMQConnectionManager;
use std::time::Duration;

const PAYLOAD: &[u8; 13] = b"Hello, World!";
const QUEUE_NAME: &str = "test";

#[tokio::main]
async fn main() {
    let addr = "amqp://rmq:rmq@127.0.0.1:5672/%2f";
    let manager = RMQConnectionManager::new(
        addr.to_owned(),
        ConnectionProperties::default().with_executor(tokio_executor_trait::Tokio::current()),
    );
    let pool = Pool::<RMQConnectionManager>::builder()
        .max_open(5)
        .build(manager);

    let conn = pool.get().await.unwrap();
    let channel = conn.create_channel().await.unwrap();
    let _ = channel
        .queue_declare(
            QUEUE_NAME,
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .await
        .unwrap();

    // send messages to the queue
    println!("spawning senders...");
    for i in 0..50 {
        let send_pool = pool.clone();
        let send_props = BasicProperties::default().with_kind(format!("Sender: {}", i).into());
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(200));
            loop {
                interval.tick().await;
                let send_conn = send_pool.get().await.unwrap();
                let send_channel = send_conn.create_channel().await.unwrap();
                let confirm = send_channel
                    .basic_publish(
                        "",
                        QUEUE_NAME,
                        BasicPublishOptions::default(),
                        PAYLOAD.as_ref(),
                        send_props.clone(),
                    )
                    .await
                    .unwrap()
                    .await
                    .unwrap();
                assert_eq!(confirm, Confirmation::NotRequested);
            }
        });
    }

    // listen for incoming messages from the queue
    let mut consumer = channel
        .basic_consume(
            QUEUE_NAME,
            "my_consumer",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await
        .unwrap();

    println!("listening to messages...");
    while let Some(delivery) = consumer.next().await {
        let delivery = delivery.expect("error in consumer");
        println!("incoming message from: {:?}", delivery.properties.kind());
        channel
            .basic_ack(delivery.delivery_tag, BasicAckOptions::default())
            .await
            .expect("ack");
    }
}
