use crate::interrupt::{
    svd_reader,
    interrupt_list::{InterruptWithPeriph, InterruptList},
};
use std::fs::File;
use std::path::Path;

pub fn parse_device(svd_file: &Path, gaps: bool) {
    let file = File::open(svd_file).expect("svd file doesn't exist");
    let peripherals = svd_reader::peripherals_with_interrupts(file);
    let interrupt_list = InterruptList::new(peripherals);

    print_interrupts(&interrupt_list.ordered());

    if gaps {
        let gaps = interrupt_list.gaps();
        print_gaps(&gaps);
    }
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

fn print_gaps(gaps: &[u32]) {
    let gaps: Vec<String> = gaps.iter().map(|g| g.to_string()).collect();
    let gaps_str = gaps.join(", ");
    println!("Gaps: {}", gaps_str);
}
