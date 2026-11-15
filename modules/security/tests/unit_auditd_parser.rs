#![forbid(unsafe_code)]

use security::{AuditdParser};

#[test]
fn test_auditd_parser_basic_syscall() {
    let mut parser = AuditdParser::new();
    let log = "type=SYSCALL msg=audit(1629895200.123:456): arch=c000003e syscall=59 success=yes exit=0 a0=7ffd6cf75510 a1=7ffd6cf75230 a2=7ffd6cf75240 a3=8 items=2 ppid=1234 pid=5678 auid=1000 uid=0 gid=0 euid=0 suid=0 fsuid=0 egid=0 sgid=0 fsgid=0 tty=pts0 ses=1 comm=\"nc\" exe=\"/usr/bin/nc\" subj=unconfined_u:unconfined_r:unconfined_t:s0-s0:c0.c1023 key=(null)\ntype=PROCTITLE msg=audit(1629895200.123:456): proctitle=6E63002D65002F62696E2F626173680031302E302E302E310034343434";

    for line in log.lines() {
        parser.parse_line(line);
    }

    let event = parser.next_event().expect("Should have one complete event");

    assert_eq!(event.uid(), Some(0));
    assert_eq!(event.auid(), Some(1000));
    assert_eq!(event.success(), Some(true));
    assert_eq!(event.comm().unwrap(), "nc");
    assert_eq!(event.exe().unwrap(), "/usr/bin/nc");
    assert!(event.is_privilege_escalation()); // auid != 0, euid == 0, success == yes
}

#[test]
fn test_auditd_parser_execve_args() {
    let mut parser = AuditdParser::new();
    let log = "type=SYSCALL msg=audit(1629895200.123:457): arch=c000003e syscall=59 success=yes exit=0 a0=7ffd6cf75510 a1=7ffd6cf75230 a2=7ffd6cf75240 a3=8 items=2 ppid=1234 pid=5678 auid=0 uid=0 gid=0 euid=0 suid=0 fsuid=0 egid=0 sgid=0 fsgid=0 tty=pts0 ses=1 comm=\"rm\" exe=\"/usr/bin/rm\" subj=unconfined_u:unconfined_r:unconfined_t:s0-s0:c0.c1023 key=(null)\ntype=EXECVE msg=audit(1629895200.123:457): argc=3 a0=\"rm\" a1=\"-rf\" a2=\"/\"\ntype=PROCTITLE msg=audit(1629895200.123:457): proctitle=726D002D7266002F";

    for line in log.lines() {
        parser.parse_line(line);
    }

    let event = parser.next_event().expect("Should have one complete event");
    let args = event.get_command_line();
    assert_eq!(args, vec!["rm", "-rf", "/"]);
    assert!(event.is_suspicious_root_command());
}

#[test]
fn test_auditd_parser_user_cmd() {
    let mut parser = AuditdParser::new();
    let log = "type=USER_CMD msg=audit(1629895200.123:458): pid=12345 uid=1000 auid=1000 ses=2 subj=unconfined_u:unconfined_r:unconfined_t:s0-s0:c0.c1023 msg='cwd=\"/home/user\" cmd=63686D6F6420373737202F6574632F706173737764 exe=\"/usr/bin/sudo\" terminal=pts/1 res=success'";

    for line in log.lines() {
        parser.parse_line(line);
    }

    let event = parser.next_event().expect("Should have one complete event");
    assert_eq!(event.uid(), Some(1000));
    let args = event.get_command_line();
    assert_eq!(args, vec!["chmod 777 /etc/passwd"]);

    // not suspicious root because uid != 0 and auid != 0
    assert!(!event.is_suspicious_root_command());
}

#[test]
fn test_auditd_hex_decoding() {
    // "chmod 777 /etc/passwd"
    let hex_val = "63686D6F6420373737202F6574632F706173737764";
    assert_eq!(AuditdParser::decode_audit_value(hex_val), "chmod 777 /etc/passwd");

    // quoted string
    assert_eq!(AuditdParser::decode_audit_value("\"/bin/bash\""), "/bin/bash");

    // normal string
    assert_eq!(AuditdParser::decode_audit_value("success"), "success");
}

#[test]
fn test_suspicious_root_command_detection() {
    let mut parser = AuditdParser::new();
    let log = "type=SYSCALL msg=audit(1:1): uid=0 auid=1000 euid=0 success=yes exe=\"/bin/nc\"\ntype=EXECVE msg=audit(1:1): argc=3 a0=\"nc\" a1=\"-e\" a2=\"/bin/bash\"\ntype=EOE msg=audit(1:1):";

    for line in log.lines() {
        parser.parse_line(line);
    }

    let event = parser.next_event().expect("Should have one complete event");
    assert!(event.is_suspicious_root_command());
    assert!(event.is_privilege_escalation());
}
