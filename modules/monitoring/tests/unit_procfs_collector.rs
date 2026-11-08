#![forbid(unsafe_code)]

use monitoring::procfs_collector::{
    compute_cpu_utilization, CpuStat, DiskStats, MemInfo, NetDevStats,
};

#[test]
fn test_parse_cpu_stat() {
    let content = "\
cpu  1000 200 300 4000 500 600 700 800
cpu0 1000 200 300 4000 500 600 700 800
page 1234
";
    let cpu = CpuStat::parse(content).expect("Failed to parse CpuStat");
    assert_eq!(cpu.user, 1000);
    assert_eq!(cpu.nice, 200);
    assert_eq!(cpu.system, 300);
    assert_eq!(cpu.idle, 4000);
    assert_eq!(cpu.iowait, 500);
    assert_eq!(cpu.irq, 600);
    assert_eq!(cpu.softirq, 700);
    assert_eq!(cpu.steal, 800);
    
    assert_eq!(cpu.idle_total(), 4500);
    assert_eq!(cpu.total(), 8100);
}

#[test]
fn test_compute_cpu_utilization() {
    let mut cpu1 = CpuStat::default();
    cpu1.user = 100;
    cpu1.nice = 0;
    cpu1.system = 50;
    cpu1.idle = 850; // total = 1000, idle_total = 850
    
    let mut cpu2 = CpuStat::default();
    cpu2.user = 150;
    cpu2.nice = 0;
    cpu2.system = 100;
    cpu2.idle = 1750; // total = 2000, idle_total = 1750
    
    // total_d = 1000
    // idle_d = 900
    // util = (1000 - 900) / 1000 * 100 = 10.0
    let util = compute_cpu_utilization(&cpu1, &cpu2);
    assert!((util - 10.0).abs() < f64::EPSILON);
    
    // Same snapshot should give 0.0 to avoid NaN
    let util_same = compute_cpu_utilization(&cpu1, &cpu1);
    assert_eq!(util_same, 0.0);
}

#[test]
fn test_parse_meminfo() {
    let content = "\
MemTotal:       16315484 kB
MemFree:         8123456 kB
MemAvailable:   12345678 kB
Buffers:          210456 kB
Cached:          3456789 kB
SwapCached:            0 kB
Active:          4567890 kB
Inactive:        1234567 kB
SwapTotal:       2097148 kB
SwapFree:        1048576 kB
";
    let mem = MemInfo::parse(content);
    assert_eq!(mem.mem_total, 16315484);
    assert_eq!(mem.mem_free, 8123456);
    assert_eq!(mem.mem_available, 12345678);
    assert_eq!(mem.buffers, 210456);
    assert_eq!(mem.cached, 3456789);
    assert_eq!(mem.swap_total, 2097148);
    assert_eq!(mem.swap_free, 1048576);
}

#[test]
fn test_parse_diskstats() {
    let content = "\
   8       0 sda 100 20 500 1000 300 40 800 2000 0 150 1500
   8       1 sda1 50 10 250 500 150 20 400 1000 0 75 750
";
    let disks = DiskStats::parse_all(content);
    assert_eq!(disks.len(), 2);
    
    let sda = &disks[0];
    assert_eq!(sda.major, 8);
    assert_eq!(sda.minor, 0);
    assert_eq!(sda.device_name, "sda");
    assert_eq!(sda.reads_completed, 100);
    assert_eq!(sda.sectors_read, 500);
    assert_eq!(sda.writes_completed, 300);
    assert_eq!(sda.sectors_written, 800);
    assert_eq!(sda.io_time_ms, 150);
    
    let sda1 = &disks[1];
    assert_eq!(sda1.major, 8);
    assert_eq!(sda1.minor, 1);
    assert_eq!(sda1.device_name, "sda1");
    assert_eq!(sda1.reads_completed, 50);
    assert_eq!(sda1.sectors_read, 250);
    assert_eq!(sda1.writes_completed, 150);
    assert_eq!(sda1.sectors_written, 400);
    assert_eq!(sda1.io_time_ms, 75);
}

#[test]
fn test_parse_net_dev() {
    let content = "\
Inter-|   Receive                                                |  Transmit
 face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed
    lo: 1234567    8901    0    0    0     0          0         0  1234567    8901    0    0    0     0       0          0
  eth0: 9876543   12345   10    0    0     0          0         0  3456789    6789    5    0    0     0       0          0
";
    let nets = NetDevStats::parse_all(content);
    assert_eq!(nets.len(), 2);
    
    let lo = &nets[0];
    assert_eq!(lo.interface_name, "lo");
    assert_eq!(lo.rx_bytes, 1234567);
    assert_eq!(lo.rx_packets, 8901);
    assert_eq!(lo.rx_errs, 0);
    assert_eq!(lo.tx_bytes, 1234567);
    assert_eq!(lo.tx_packets, 8901);
    assert_eq!(lo.tx_errs, 0);
    
    let eth0 = &nets[1];
    assert_eq!(eth0.interface_name, "eth0");
    assert_eq!(eth0.rx_bytes, 9876543);
    assert_eq!(eth0.rx_packets, 12345);
    assert_eq!(eth0.rx_errs, 10);
    assert_eq!(eth0.tx_bytes, 3456789);
    assert_eq!(eth0.tx_packets, 6789);
    assert_eq!(eth0.tx_errs, 5);
}
