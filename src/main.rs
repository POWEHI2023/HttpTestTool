
use std::{fmt::format, time::Duration};

use async_std:: { net::TcpListener, prelude::*, task::{sleep, spawn} };
use futures::stream::StreamExt;
use async_std::io::{Read, Write};
use std::collections::HashMap;

const TCP_PACKAGE_LENGTH: usize = 1024;

#[async_std::main]
async fn main() {
     let listener = TcpListener::bind("127.0.0.1:7878").await.unwrap();
     listener.incoming()
          .for_each_concurrent(None, |tcpstream| async move {
               let tcpstream = tcpstream.unwrap();
               spawn(handle_connection(tcpstream));
          })
          .await;
}

#[derive(Debug)]
struct HttpLine {
     operation: String,
     http_version: String,
     request_url: String,
}

impl HttpLine {
     // for debugging
     fn new() -> Self {
          HttpLine {
               operation: String::from("POST"),
               http_version: String::from("http/1.1"),
               request_url: String::from("url"),
          }
     }

     // all args constructor
     fn new_full(operation: String, http_version: String, request_url: String) -> Self {
          HttpLine { operation, http_version, request_url }
     }
}

#[derive(Debug)]
struct HttpContent {
     first_line: HttpLine,
     heads: HashMap<String, String>,
     body: String,

     content_length: usize,
}

impl HttpContent {
     fn new() -> Self {
          HttpContent{
               first_line: HttpLine::new(),
               heads: HashMap::new(),
               body: String::new(),

               content_length: 0,
          }
     }
}

// for each connection, handle it
async fn handle_connection(mut stream: impl Read + Write + Unpin) {
     let mut buffer = [0 as u8; TCP_PACKAGE_LENGTH];
     let mut start = 0;
     let mut httpcontent: HttpContent = HttpContent::new();
     let mut is_end = false;

     loop {
          catch_and_parse_http(&mut stream, &mut buffer, &mut start, &mut httpcontent, &mut is_end).await;
          process_http_content(&mut httpcontent, &mut stream).await;

          println!("is_end? {is_end}");
          if is_end == true { break; }
     }
}

/**
 * 
 */
async fn catch_and_parse_http(
     stream: &mut (impl Read + Write + Unpin), 
     buf: &mut [u8], 
     start: &mut usize, 
     httpcontent: &mut HttpContent,
     _end: &mut bool
) {
     // n is readed length
     let n = stream.read(&mut buf[*start..]).await.unwrap();
     if n == 0 && *start == 0 { 
          *_end = true;
          return;
     } 
     /*else if n <= TCP_PACKAGE_LENGTH - *start {
          // reach some data and fill the buffer, next start position is 0
          *start = 0;
     } else {
          // data is not fill the buffer
          *start = n;
     }*/

     let content = String::from_utf8(buf.to_vec()).unwrap();
     // split into different parts
     // line \r\n [[..]\r\n].. \r\n body..
     let content: Vec<&str> = content.split("\r\n").collect();

     // todo!: get content_length (is 0 or not)
     // parse into httpline / content_length / body
     // is body is not enough, just set the content_length of body, the last process deliver to process_htto_content()

     // content can definitely parse out content_length, TCP_PACKAGE_LENGTH is enough for httpline and httphead
     httpcontent.first_line = parse_http_line(content[0]);
     let mut stop = parse_http_heads(&content, &mut httpcontent.heads);

     httpcontent.content_length = if let Some(val) = httpcontent.heads.get("Content-Length") {
          val.parse().unwrap()
     } else { 0 };

     // TCP_PACKAGE_LENGTH - start = empty size of the last part
     // total body - empty part = real part of body
     httpcontent.body =  if httpcontent.content_length > 0 && stop < content.len() 
                         { String::from(&content[stop][ 0..httpcontent.content_length ]) }
                         else { String::from("") };
     
     // DEBUG -- display infomation of httpcontent
     println!("{:#?}", httpcontent);


     // clear and mark the end
     if n + *start >= TCP_PACKAGE_LENGTH {
          *start = 0;
          *_end = true;
          return;
     }


     if httpcontent.content_length > 0 {
          stop += 2;
     }
     *start = 0;
     for i in stop..content.len() {
          *start += content[i].len();
     }

     let mut i = 0;
     let mut j = *start;
     while j < buf.len() {
          buf[i] = buf[j];
          i += 1;
          j += 1;
     }
     while i < buf.len() {
          buf[i] = 0;
          i += 1;
     }

     if *start == 0 { *_end = true; }
     
}

/**
 * 
 */
fn parse_http_line(line: &str) -> HttpLine {
     // GET/POST URL HTTP/1.1
     let vec: Vec<&str> = line.split(" ").collect();
     assert!(vec.len() >= 3, "parse http line failed... length less than 3(POST URL VERSION)");
     HttpLine::new_full(String::from(vec[0]), String::from(vec[2]), String::from(vec[1]))
}

/**
 * 
 */
fn parse_http_heads(content: &Vec<&str>, ret: &mut HashMap<String, String>) -> usize {
     for i in 1..content.len() {
          let s = content[i];
          if s == "" { return i + 1; }

          if let Some(pos) = s.find(':') {
               ret.insert(String::from(&s[0..pos]), String::from(&s[pos + 2..]));
          } else { return i; }
     }
     
     content.len()
}



async fn process_http_content(content: &mut HttpContent, stream: &mut (impl Read + Write + Unpin)) {

     if content.body.len() < content.content_length {
          let remain_length = content.content_length - content.body.len();
          let mut buffer = vec![0 as u8; remain_length];
          let n = stream.read(&mut buffer.as_mut_slice()).await.unwrap();

          if n != remain_length {
               todo!("There is more content waits to recv......");
          }

          // complete body
          content.body += &String::from_utf8(buffer.to_vec()).unwrap();
     }

     // process and get response
     let response = parse_body_and_process_main(&content.body, &content.heads);

     // send response to client and end current interaction
     println!("response to client: {response}");
     stream.write_all(response.as_bytes()).await.unwrap();
     stream.flush().await.unwrap();

}

/**
 * TODO: resp_body response for _body
 */
fn parse_body_and_process_main(_body: &str, heads:&HashMap<String, String>) -> String {
     let resp_body = String::from("Hello World...");
     format!("HTTP/1.1 200 OK\r\n{}\r\n{resp_body}", format_http_heads(heads))
}

/**
 * DEBUG -- enpty heads
 */
fn format_http_heads(heads: &HashMap<String, String>) -> String {
     String::from("")
}
