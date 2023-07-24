use std::net::{TcpListener};
use web_server::threadpool::ThreadPool;
use web_server;
use tokio::runtime::Runtime;

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("0.0.0.0:8080").unwrap();
    let pool = ThreadPool::new(4);

    for stream in listener.incoming() {
        let stream = stream.unwrap();

        pool.execute(|| {
            let rt = Runtime::new().unwrap();
            rt.block_on(async {
                web_server::handle_connection(stream).await;
            })
        })
    }
}
