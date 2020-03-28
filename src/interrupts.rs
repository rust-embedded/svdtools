use quick_xml::de;
use serde::Deserialize;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

#[derive(Deserialize, Debug)]
struct Svd {
    peripherals: PeripheralList,
}

#[derive(Deserialize, Debug)]
struct PeripheralList {
    peripheral: Vec<Peripheral>,
}

#[derive(Deserialize, Debug)]
struct Peripheral {
    name: String,
    interrupt: Option<Vec<Interrupt>>,
}

#[derive(Deserialize, Debug)]
struct Interrupt {
    name: String,
    description: Option<String>,
    value: u32,
}

pub fn parse_device(svd_file: PathBuf, gaps: bool) {
    println!("{:?}, {}", svd_file, gaps);
    let file = File::open(svd_file).expect("svd file doesn't exist");
    let reader = BufReader::new(file);

    let svd: Svd = de::from_reader(reader).expect("svd not formatted correctly");

    let peripheral_list = svd.peripherals.peripheral;
    let mut interrupt_list: Vec<_> = peripheral_list
        .into_iter()
        .filter(|p| p.interrupt.is_some())
        .flat_map(|p| {
            let name = p.name;
            p.interrupt
                .unwrap()
                .into_iter()
                .map(move |i| (name.clone(), i))
        })
        .collect();
    interrupt_list.sort_by_key(|i| i.1.value);

    for (peripheral, interrupt) in &interrupt_list {
        let description: String = match &interrupt.description {
            Some(desc) => desc.clone(),
            None => "".to_string(),
        };
        let description = description.replace("\r\n", " ").replace("\n", " ");
        println!(
            "{} {}: {} (in {})",
            interrupt.value, interrupt.name, description, peripheral
        );
    }

    if gaps {
        let mut gaps = Vec::new();
        let mut interrupt_list_iter = interrupt_list.iter().peekable();
        while let Some(i) = interrupt_list_iter.next() {
            let curr_num = i.1.value;
            if let Some(next_interrupt) = interrupt_list_iter.peek() {
                let next_num = next_interrupt.1.value;
                for k in (curr_num + 1)..next_num {
                    gaps.push(k);
                }
            }
        }
        println!("Gaps: {:?}", gaps);
    }
}
