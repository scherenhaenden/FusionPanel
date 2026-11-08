#![forbid(unsafe_code)]

use std::process::Stdio;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

#[derive(Clone)]
pub struct Sha256 {
    state: [u32; 8],
    count: u64,
    buffer: [u8; 64],
}

impl Sha256 {
    pub fn new() -> Self {
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ],
            count: 0,
            buffer: [0; 64],
        }
    }

    pub fn update(&mut self, data: &[u8]) {
        let mut i = 0;
        let len = data.len();
        let buffer_len = (self.count % 64) as usize;
        self.count += len as u64;

        if buffer_len > 0 {
            let space = 64 - buffer_len;
            if len < space {
                self.buffer[buffer_len..buffer_len + len].copy_from_slice(data);
                return;
            }
            self.buffer[buffer_len..64].copy_from_slice(&data[..space]);
            self.process_block(self.buffer);
            i += space;
        }

        while i + 64 <= len {
            let mut block = [0u8; 64];
            block.copy_from_slice(&data[i..i + 64]);
            self.process_block(block);
            i += 64;
        }

        if i < len {
            self.buffer[..len - i].copy_from_slice(&data[i..]);
        }
    }

    fn process_block(&mut self, block: [u8; 64]) {
        let mut w = [0u32; 64];
        for j in 0..16 {
            w[j] = u32::from_be_bytes([
                block[j * 4],
                block[j * 4 + 1],
                block[j * 4 + 2],
                block[j * 4 + 3],
            ]);
        }

        for j in 16..64 {
            let s0 = w[j - 15].rotate_right(7) ^ w[j - 15].rotate_right(18) ^ (w[j - 15] >> 3);
            let s1 = w[j - 2].rotate_right(17) ^ w[j - 2].rotate_right(19) ^ (w[j - 2] >> 10);
            w[j] = w[j - 16]
                .wrapping_add(s0)
                .wrapping_add(w[j - 7])
                .wrapping_add(s1);
        }

        let mut a = self.state[0];
        let mut b = self.state[1];
        let mut c = self.state[2];
        let mut d = self.state[3];
        let mut e = self.state[4];
        let mut f = self.state[5];
        let mut g = self.state[6];
        let mut h = self.state[7];

        const K: [u32; 64] = [
            0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
            0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
            0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
            0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
            0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
            0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
            0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
            0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
            0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
            0xc67178f2,
        ];

        for j in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[j])
                .wrapping_add(w[j]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
        self.state[5] = self.state[5].wrapping_add(f);
        self.state[6] = self.state[6].wrapping_add(g);
        self.state[7] = self.state[7].wrapping_add(h);
    }

    pub fn finalize(mut self) -> String {
        let bit_len = self.count * 8;
        let buffer_len = (self.count % 64) as usize;

        self.buffer[buffer_len] = 0x80;
        for i in buffer_len + 1..64 {
            self.buffer[i] = 0;
        }

        if buffer_len >= 56 {
            let block = self.buffer;
            self.process_block(block);
            for i in 0..64 {
                self.buffer[i] = 0;
            }
        }

        let bit_len_bytes = bit_len.to_be_bytes();
        self.buffer[56..64].copy_from_slice(&bit_len_bytes);

        let block = self.buffer;
        self.process_block(block);

        let mut hash = String::with_capacity(64);
        for &s in &self.state {
            use std::fmt::Write;
            write!(&mut hash, "{:08x}", s).unwrap();
        }
        hash
    }
}

pub async fn stream_dump(
    utility: &str,
    args: &[&str],
    output_path: &str,
) -> std::io::Result<String> {
    let mut cmd = Command::new(utility);
    cmd.args(args);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            panic!("RESULT=BLOCKED: Database dump utility is missing");
        }
        Err(e) => return Err(e),
    };

    let dump_stdout = child.stdout.take().unwrap();
    let std_dump_stdout: std::process::Stdio = dump_stdout.try_into().unwrap();

    let mut zstd_cmd = Command::new("zstd");
    zstd_cmd.arg("-c").arg("-3");
    zstd_cmd.stdin(Stdio::from(std_dump_stdout));
    zstd_cmd.stdout(Stdio::piped());
    zstd_cmd.stderr(Stdio::piped());

    let mut zstd_child = match zstd_cmd.spawn() {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            panic!("RESULT=BLOCKED: zstd utility is missing");
        }
        Err(e) => return Err(e),
    };

    let mut zstd_stdout = zstd_child.stdout.take().unwrap();

    let mut file_options = tokio::fs::OpenOptions::new();
    file_options.write(true).create(true).truncate(true);

    #[cfg(unix)]
    {
        file_options.mode(0o600);
    }

    let mut file = file_options.open(output_path).await?;
    let mut hasher = Sha256::new();

    let mut buffer = [0u8; 8192];
    loop {
        let n = zstd_stdout.read(&mut buffer).await?;
        if n == 0 {
            break;
        }
        let chunk = &buffer[..n];
        hasher.update(chunk);
        file.write_all(chunk).await?;
    }

    let zstd_status = zstd_child.wait().await?;
    if !zstd_status.success() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("zstd failed with status: {}", zstd_status),
        ));
    }

    let dump_status = child.wait().await?;
    if !dump_status.success() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("dump utility failed with status: {}", dump_status),
        ));
    }

    Ok(hasher.finalize())
}
