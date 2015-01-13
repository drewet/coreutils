#![crate_name = "wc"]
#![allow(unstable)]

/*
 * This file is part of the uutils coreutils package.
 *
 * (c) Boden Garman <bpgarman@gmail.com>
 *
 * For the full copyright and license information, please view the LICENSE
 * file that was distributed with this source code.
 */

extern crate getopts;
extern crate libc;

use std::ascii::AsciiExt;
use std::str::from_utf8;
use std::io::{print, File, BufferedReader};
use std::io::fs::PathExtensions;
use std::io::stdio::stdin_raw;
use std::result::Result as StdResult;
use getopts::Matches;

#[path = "../common/util.rs"]
#[macro_use]
mod util;

struct Result {
    filename: String,
    bytes: usize,
    chars: usize,
    lines: usize,
    words: usize,
    max_line_length: usize,
}

static NAME: &'static str = "wc";

pub fn uumain(args: Vec<String>) -> isize {
    let program = args[0].clone();
    let opts = [
        getopts::optflag("c", "bytes", "print the byte counts"),
        getopts::optflag("m", "chars", "print the character counts"),
        getopts::optflag("l", "lines", "print the newline counts"),
        getopts::optflag("L", "max-line-length", "print the length of the longest line"),
        getopts::optflag("w", "words", "print the word counts"),
        getopts::optflag("h", "help", "display this help and exit"),
        getopts::optflag("V", "version", "output version information and exit"),
    ];

    let matches = match getopts::getopts(args.tail(), &opts) {
        Ok(m) => m,
        Err(f) => {
            crash!(1, "Invalid options\n{}", f)
        }
    };

    if matches.opt_present("help") {
        println!("Usage:");
        println!("  {0} [OPTION]... [FILE]...", program);
        println!("");
        print(getopts::usage("Print newline, word and byte counts for each FILE", &opts).as_slice());
        println!("");
        println!("With no FILE, or when FILE is -, read standard input.");
        return 0;
    }

    if matches.opt_present("version") {
        println!("wc 1.0.0");
        return 0;
    }

    let mut files = matches.free.clone();
    if files.is_empty() {
        files = vec!("-".to_string());
    }

    match wc(files, &matches) {
        Ok(()) => ( /* pass */ ),
        Err(e) => return e
    }

    0
}

const CR: u8 = '\r' as u8;
const LF: u8 = '\n' as u8;
const SPACE: u8 = ' ' as u8;
const TAB: u8 = '\t' as u8;
const SYN: u8 = 0x16 as u8;
const FF: u8 = 0x0C as u8;

#[inline(always)]
fn is_word_seperator(byte: u8) -> bool {
    byte == SPACE || byte == TAB || byte == CR || byte == SYN || byte == FF
}

pub fn wc(files: Vec<String>, matches: &Matches) -> StdResult<(), isize> {
    let mut total_line_count: usize = 0;
    let mut total_word_count: usize = 0;
    let mut total_char_count: usize = 0;
    let mut total_byte_count: usize = 0;
    let mut total_longest_line_length: usize = 0;

    let mut results = vec!();
    let mut max_str_len: usize = 0;

    for path in files.iter() {
        let mut reader = try!(open(path.as_slice()));

        let mut line_count: usize = 0;
        let mut word_count: usize = 0;
        let mut byte_count: usize = 0;
        let mut char_count: usize = 0;
        let mut current_char_count: usize = 0;
        let mut longest_line_length: usize = 0;

        loop {
            // reading from a TTY seems to raise a condition on, rather than return Some(0) like a file.
            // hence the option wrapped in a result here
            match reader.read_until(LF) {
                Ok(raw_line) => {
                    // GNU 'wc' only counts lines that end in LF as lines
                    if *raw_line.last().unwrap() == LF {
                        line_count += 1;
                    }

                    byte_count += raw_line.len();

                    // try and convert the bytes to UTF-8 first
                    match from_utf8(raw_line.as_slice()) {
                        Ok(line) => {
                            word_count += line.words().count();
                            current_char_count = line.chars().count();
                            char_count += current_char_count;
                        },
                        Err(..) => {
                            word_count += raw_line.as_slice().split(|&x| is_word_seperator(x)).count();
                            for byte in raw_line.iter() {
                                match byte.is_ascii() {
                                    true => {
                                        current_char_count += 1;
                                    }
                                    false => { }
                                }
                            }
                            char_count += current_char_count;
                        }
                    }

                    if current_char_count > longest_line_length {
                        // we subtract one here because `line.len()` includes the LF
                        // matches GNU 'wc' behaviour
                        longest_line_length = current_char_count - 1;
                    }
                },
                _ => break
            }

        }

        results.push(Result {
            filename: path.to_string(),
            bytes: byte_count,
            chars: char_count,
            lines: line_count,
            words: word_count,
            max_line_length: longest_line_length,
        });

        total_line_count += line_count;
        total_word_count += word_count;
        total_char_count += char_count;
        total_byte_count += byte_count;

        if longest_line_length > total_longest_line_length {
            total_longest_line_length = longest_line_length;
        }

        // used for formatting
        max_str_len = total_byte_count.to_string().len();
    }

    for result in results.iter() {
        print_stats(result.filename.as_slice(), result.lines, result.words, result.chars, result.bytes, result.max_line_length, matches, max_str_len);
    }

    if files.len() > 1 {
        print_stats("total", total_line_count, total_word_count, total_char_count, total_byte_count, total_longest_line_length, matches, max_str_len);
    }

    Ok(())
}

fn print_stats(filename: &str, line_count: usize, word_count: usize, char_count: usize,
    byte_count: usize, longest_line_length: usize, matches: &Matches, max_str_len: usize) {
    if matches.opt_present("lines") {
        print!("{:1$}", line_count, max_str_len);
    }
    if matches.opt_present("words") {
        print!("{:1$}", word_count, max_str_len);
    }
    if matches.opt_present("bytes") {
        print!("{:1$}", byte_count, max_str_len);
    }
    if matches.opt_present("chars") {
        print!("{:1$}", char_count, max_str_len);
    }
    if matches.opt_present("max-line-length") {
        print!("{:1$}", longest_line_length, max_str_len);
    }

    // defaults
    if !matches.opt_present("bytes")
        && !matches.opt_present("chars")
        && !matches.opt_present("lines")
        && !matches.opt_present("words")
        && !matches.opt_present("max-line-length") {
        print!("{:1$}", line_count, max_str_len);
        print!("{:1$}", word_count, max_str_len + 1);
        print!("{:1$}", byte_count, max_str_len + 1);
    }

    if filename != "-" {
        println!(" {}", filename.as_slice());
    }
    else {
        println!("");
    }
}

fn open(path: &str) -> StdResult<BufferedReader<Box<Reader+'static>>, isize> {
    if "-" == path {
        let reader = Box::new(stdin_raw()) as Box<Reader>;
        return Ok(BufferedReader::new(reader));
    }

    let fpath = Path::new(path);
    if fpath.is_dir() {
        show_info!("{}: is a directory", path);
    }
    match File::open(&fpath) {
        Ok(fd) => {
            let reader = Box::new(fd) as Box<Reader>;
            Ok(BufferedReader::new(reader))
        }
        Err(e) => {
            show_error!("wc: {}: {}", path, e);
            Err(1)
        }
    }
}
