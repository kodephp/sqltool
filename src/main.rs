use clap::Parser;
use sqltool::{init, Args, execute};

#[tokio::main]
async fn main() {
    // 初始化库
    init();
    
    // 解析命令行参数
    let args = Args::parse();
    
    // 执行命令
    if let Err(e) = execute(args).await {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
