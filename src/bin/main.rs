use std::io::Read;

use tokio::io::AsyncReadExt;

#[tokio::main]
async fn main() {
    let mut c = tun::Configuration::default();

    c.address((10, 0, 0, 0)).netmask((255, 255, 255, 0)).up();

    #[cfg(target_os = "linux")]
    c.platform(|config| {
        config.packet_information(true);
    });

    let mut dev = tun::create_as_async(&c).unwrap();

    let mut buf = [0; 4096];

    loop {
        let amount = dev.read(&mut buf).await.unwrap();
        
        println!("{:?}", &buf[0..amount]);
    }
}
