#![forbid(unsafe_code)]

/// Represents a bandwidth cap with limit and burst settings in Mbit/s.
#[derive(Debug, Clone, PartialEq)]
pub struct BandwidthCap {
    pub limit_mbit: u32,
    pub burst_mbit: u32,
}

/// Represents a traffic shaping policy to be applied to a specific interface.
#[derive(Debug, Clone, PartialEq)]
pub struct TrafficShapingPolicy {
    pub interface: String,
    pub ingress: Option<BandwidthCap>,
    pub egress: Option<BandwidthCap>,
}

/// A command to run via `tc`
#[derive(Debug, Clone, PartialEq)]
pub struct TcCommand {
    pub args: Vec<String>,
}

impl TcCommand {
    fn new(args: Vec<&str>) -> Self {
        Self {
            args: args.into_iter().map(|s| s.to_string()).collect(),
        }
    }
}

pub struct TrafficShaper;

impl TrafficShaper {
    /// Generates the necessary `tc` command arguments to enforce the given policy.
    pub fn generate_tc_args(policy: &TrafficShapingPolicy) -> Vec<TcCommand> {
        let mut cmds = Vec::new();
        let iface = &policy.interface;

        // Clear existing rules on the main interface
        cmds.push(TcCommand::new(vec!["qdisc", "del", "dev", iface, "root"]));
        cmds.push(TcCommand::new(vec!["qdisc", "del", "dev", iface, "ingress"]));

        // --- Egress ---
        if let Some(egress) = &policy.egress {
            // Root qdisc (HTB)
            cmds.push(TcCommand::new(vec![
                "qdisc", "add", "dev", iface, "root", "handle", "1:", "htb", "default", "11",
            ]));

            // HTB Class
            let limit = format!("{}mbit", egress.limit_mbit);
            let burst = format!("{}mbit", egress.burst_mbit);
            cmds.push(TcCommand::new(vec![
                "class", "add", "dev", iface, "parent", "1:", "classid", "1:11",
                "htb", "rate", &limit, "ceil", &burst,
            ]));

            // Leaf qdisc (fq_codel)
            cmds.push(TcCommand::new(vec![
                "qdisc", "add", "dev", iface, "parent", "1:11", "handle", "10:", "fq_codel",
            ]));
        }

        // --- Ingress ---
        if let Some(ingress) = &policy.ingress {
            // Add ingress qdisc to the main interface
            cmds.push(TcCommand::new(vec![
                "qdisc", "add", "dev", iface, "handle", "ffff:", "ingress",
            ]));

            // In our case, for ingress shaping we might redirect to an IFB device, or use police.
            // But we need to use HTB and fq_codel, so we use an ifb device.
            let ifb_dev = format!("ifb-{}", iface);
            // In a real scenario we'd need to limit length of ifb_dev. We assume ifb is set up.

            // Redirect traffic from ingress to IFB
            cmds.push(TcCommand::new(vec![
                "filter", "add", "dev", iface, "parent", "ffff:", "protocol", "all", "u32", "match", "u32", "0", "0", "action", "mirred", "egress", "redirect", "dev", &ifb_dev,
            ]));

            // Clear existing rules on IFB
            cmds.push(TcCommand::new(vec!["qdisc", "del", "dev", &ifb_dev, "root"]));

            // Root qdisc (HTB) on IFB
            cmds.push(TcCommand::new(vec![
                "qdisc", "add", "dev", &ifb_dev, "root", "handle", "1:", "htb", "default", "11",
            ]));

            // HTB Class on IFB
            let limit = format!("{}mbit", ingress.limit_mbit);
            let burst = format!("{}mbit", ingress.burst_mbit);
            cmds.push(TcCommand::new(vec![
                "class", "add", "dev", &ifb_dev, "parent", "1:", "classid", "1:11",
                "htb", "rate", &limit, "ceil", &burst,
            ]));

            // Leaf qdisc (fq_codel) on IFB
            cmds.push(TcCommand::new(vec![
                "qdisc", "add", "dev", &ifb_dev, "parent", "1:11", "handle", "10:", "fq_codel",
            ]));
        }

        cmds
    }
}
