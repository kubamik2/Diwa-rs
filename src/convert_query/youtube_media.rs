use reqwest::{ Client, Response };
use std::io::{ Read, Cursor, Seek, Write };
use std::process::Stdio;
use songbird::input::reader::MediaSource;
pub struct YoutubeStreamMediaSource {
    decoded_buffer_reader: Option<Cursor<Vec<u8>>>,
    encoded_buffer: Vec<u8>,
    pub content_length: u64,
    decoded_buffer_size: u64,
    bytes_read: u64,
    response: Response
}

impl YoutubeStreamMediaSource {
    pub async fn new(video_id: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let client = Client::new();
        let id = rustube::Id::from_string(video_id.to_owned())?;
        let video = rustube::Video::from_id(id).await?;
        let stream = video.best_audio().unwrap();
        let stream_url = stream.signature_cipher.url.as_str().to_owned();
        let content_length = stream.content_length().await?;
        let response = client.get(&stream_url).send().await.unwrap();
        Ok(Self { decoded_buffer_reader: None, encoded_buffer: vec![], content_length, decoded_buffer_size: 0, bytes_read: 0, response })
    }

    pub async fn test2(&mut self) {
        let ffmpeg_args = [
            "-i",
            "./temp",
            "-f",
            "s16le",
            "-ac",
            "2",
            "-ar",
            "48000",
            "-acodec",
            "pcm_f32le",
            "-",
        ];
        if let Some(chunk) = self.response.chunk().await.unwrap() {
            for byte in chunk {
                self.encoded_buffer.push(byte);
            }
        } else {
            return ;
        }

        let mut file = std::fs::File::create("./temp").unwrap();
        file.write_all(&self.encoded_buffer).unwrap();

        let ffmpeg_out = std::process::Command::new("ffmpeg")
            .args(&ffmpeg_args)
            .stdin(Stdio::piped())
            .stderr(Stdio::null())
            .stdout(Stdio::piped())
            .output().unwrap();
        std::fs::remove_file("./temp");

        self.decoded_buffer_size = ffmpeg_out.stdout.len() as u64;
        let mut old_position = 0;
        if let Some(buffer) = self.decoded_buffer_reader.as_mut() {
            old_position = buffer.position();
        }
        self.decoded_buffer_reader = Some(Cursor::new(ffmpeg_out.stdout));
        self.decoded_buffer_reader.as_mut().unwrap().set_position(old_position);
    }
}

impl Read for YoutubeStreamMediaSource {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        if self.bytes_read + 16_384 > self.decoded_buffer_size {
            let now = std::time::Instant::now();
            runtime.block_on(self.test2());
            dbg!(now.elapsed().as_millis());
        }
        if let Some(buffer) = &mut self.decoded_buffer_reader {
            let bytes_read = buffer.read(buf)?;
            self.bytes_read += bytes_read as u64;
            return Ok(bytes_read);
        }
        
        Err(std::io::Error::from_raw_os_error(0))
    }
}

impl Seek for YoutubeStreamMediaSource {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        if let Some(buffer) = &mut self.decoded_buffer_reader {
            return buffer.seek(pos);
        }
        Err(std::io::Error::from_raw_os_error(0))
    }
}

impl MediaSource for YoutubeStreamMediaSource {
    fn byte_len(&self) -> Option<u64> {
        Some(self.content_length)
    }

    fn is_seekable(&self) -> bool {
        false
    }
}