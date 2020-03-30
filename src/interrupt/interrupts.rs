use crate::interrupt::{
    svd_reader,
    svd_reader::{Interrupt, Peripheral},
};
use std::fs::File;
use std::path::Path;

struct InterruptWithPeriph {
    pub peripheral: String,
    pub interrupt: Interrupt,
}

pub fn parse_device(svd_file: &Path, gaps: bool) {
    let file = File::open(svd_file).expect("svd file doesn't exist");
    let peripherals = svd_reader::peripherals_with_interrupts(file);
    let interrupt_list = get_ordered_interrupts(peripherals);

    print_interrupts(&interrupt_list);

    if gaps {
        let gaps = get_gaps(&interrupt_list);
        print_gaps(&gaps);
    }
}

fn get_ordered_interrupts(
    peripherals: impl Iterator<Item = Peripheral>,
) -> Vec<InterruptWithPeriph> {
    let mut interrupt_list: Vec<_> = peripherals
        .flat_map(|p| {
            let peripheral = p.name;
            p.interrupt.into_iter().map(move |i| InterruptWithPeriph {
                peripheral: peripheral.clone(),
                interrupt: i,
            })
        })
        .collect();
    interrupt_list.sort_by_key(|i| i.interrupt.value);
    interrupt_list
}

fn print_interrupts(interrupt_list: &[InterruptWithPeriph]) {
    for InterruptWithPeriph {
        peripheral,
        interrupt,
    } in interrupt_list
    {
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
}

fn get_gaps(interrupt_list: &[InterruptWithPeriph]) -> Vec<u32> {
    let mut gaps = Vec::new();
    let mut interrupt_list_iter = interrupt_list.iter().peekable();
    while let Some(i) = interrupt_list_iter.next() {
        let curr_num = i.interrupt.value;
        if let Some(i) = interrupt_list_iter.peek() {
            let next_num = i.interrupt.value;
            for k in (curr_num + 1)..next_num {
                gaps.push(k);
            }
        }
    }
    gaps
}

fn print_gaps(gaps: &[u32]) {
    let gaps: Vec<String> = gaps.iter().map(|g| g.to_string()).collect();
    let gaps_str = gaps.join(", ");
    println!("Gaps: {}", gaps_str);
}
