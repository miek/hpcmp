use clap::{Arg, App};
use log::{LevelFilter, debug, trace};
use simple_logger::SimpleLogger;

#[derive(Copy, Clone, Debug, PartialEq)]
enum Code {
    Command(u8),
    Value(u8),
    Index(usize),
}

impl Code {
    fn from_u32(code: u32) -> Code {
        use Code::*;
        match code {
            c if c < 0x8   => Command(c as u8),
            c if c > 0x107 => Index((c - 0x108) as usize),
            c              => Value((c - 8) as u8),
        }
    }
}

#[derive(Debug)]
struct DictionaryEntry {
    value: u8,
    next:  Code,
}

struct Reader {
    bit_buffer: u32,
    available: u8,
    read_width: u8,
}

impl Reader {
    fn new() -> Reader {
        Reader{
            bit_buffer: 0,
            available: 0,
            read_width: 9,
        }
    }

    fn read(&mut self, stream: &mut impl std::io::Read) -> Code {
        // Read from the input stream until enough bits are available
        let mut buf = [0u8; 1];
        while self.available < self.read_width {
            if stream.read(&mut buf).unwrap() != 1 {
                panic!();
            }
            self.bit_buffer = self.bit_buffer | ((buf[0] as u32) << self.available);
            self.available += 8;
        }

        // Read n bits
        let data = self.bit_buffer & ((1 << self.read_width) - 1);
        self.bit_buffer >>= self.read_width;
        self.available -= self.read_width;
    
        let code = Code::from_u32(data);
        use Code::*;
        match code {
            // Reset
            Command(1) => {
                self.bit_buffer = 0;
                self.available = 0;
                self.read_width = 9;
            },
            // Increase code width
            Command(2) => {
                self.read_width += 1;
            },
            Command(3) => {
                self.bit_buffer = 0;
                self.available = 0;
            },
            _ => {}
        }
    
        debug!("read: {:?}", code);
    
        return code;
    }
}

fn decompress(stream: &mut impl std::io::Read) -> Vec<u8> {
    let mut reader = Reader::new();

    use Code::*;
    if reader.read(stream) != Command(1) {
        panic!("Start marker not found.");
    }

    let mut prev;
    let mut prev_data;
    let mut prev_scratch_len = 0;
    let mut out = vec![];

    loop {
        debug!("dict: reset");
        let mut dictionary: Vec<DictionaryEntry> = vec![];

        let code = reader.read(stream);
        if let Value(data) = code {
            out.push(data);
            prev_data = data;
        } else {
            panic!("First byte not Value");
        }
        prev = code;

        loop {
            let code = reader.read(stream);

            match code {
                Command(c) => {
                    match c {
                        // Reset
                        1 => {
                            // Break to outer loop to clear dictionary
                            break;
                        },
                        // End of file
                        3 => {
                            if let Value(last) = reader.read(stream) {
                                out.push(last);
                                return out;
                            } else {
                                panic!("Final byte not Value");
                            }
                        },
                        _ => (),
                    }
                },
                mut c => {
                    let mut scratch = vec![];
                    if let Index(p) = c {
                        if p == dictionary.len() {
                            scratch.insert(0, prev_data);
                            c = prev;
                        }
                    }
                    while let Index(p) = c {
                        scratch.insert(0, dictionary[p].value);
                        c = dictionary[p].next;
                    }
                    if let Value(d) = c {
                        scratch.insert(0, d);
                        prev_data = d;
                        if prev_scratch_len < 0x80 && dictionary.len() != 0x1000 {
                            dictionary.push(DictionaryEntry{ value: d, next: prev });
                            debug!("dict: insert {} {:?}", dictionary.len()-1, dictionary[dictionary.len()-1]);
                        }
                    } else {
                        panic!("Index to non-Value");
                    }
                    trace!("0x{:x} {:?}", out.len(), scratch);
                    prev_scratch_len = scratch.len();
                    out.append(&mut scratch);

                    prev = code;
                }
            }
        }
    }
}

fn main() {
    let matches = App::new("hpcmp")
        .arg(Arg::with_name("input")
             .required(true))
        .arg(Arg::with_name("output")
             .required(true))
        .arg(Arg::with_name("v")
             .short("v")
             .multiple(true)
             .help("Sets the level of verbosity"))
        .get_matches();

    let log_level = match matches.occurrences_of("v") {
        0     => LevelFilter::Error,
        1     => LevelFilter::Info,
        2     => LevelFilter::Debug,
        3 | _ => LevelFilter::Trace,
    };

    SimpleLogger::new()
        .with_level(log_level)
        .init()
        .unwrap();

    let input  = matches.value_of("input").unwrap();
    let output = matches.value_of("output").unwrap();

    let mut input_stream = std::fs::File::open(input).unwrap();
    std::fs::write(output, decompress(&mut input_stream)).unwrap();
}
