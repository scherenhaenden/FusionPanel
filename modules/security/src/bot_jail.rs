#![forbid(unsafe_code)]

use std::fmt::Write;

/// Configurations for generating an Nginx bad bots jail.
#[derive(Debug, Clone)]
pub struct NginxBadBotJail {
    pub name: String,
    pub log_paths: Vec<String>,
    pub maxretry: u32,
    pub findtime: u32, // in seconds
    pub bantime: u32,  // in seconds
    pub bad_bots: Vec<String>,
}

impl Default for NginxBadBotJail {
    fn default() -> Self {
        Self {
            name: "nginx-badbots".to_string(),
            log_paths: vec![
                "/var/log/nginx/access.log".to_string(),
                "/var/log/nginx/*access.log".to_string(),
            ],
            maxretry: 2,
            findtime: 600,
            bantime: 86400,
            bad_bots: vec![
                "Aboundex".to_string(),
                "80legs".to_string(),
                "360Spider".to_string(),
                "^b0t$".to_string(),
                "AhrefsBot".to_string(),
                "Alexibot".to_string(),
                "asterias".to_string(),
                "attach".to_string(),
                "BackDoorBot".to_string(),
                "BackWeb".to_string(),
                "Bandit".to_string(),
                "Barkrowler".to_string(),
                "BatchFTP".to_string(),
                "Bigfoot".to_string(),
                "Black.Hole".to_string(),
                "BlackWidow".to_string(),
                "BlowFish".to_string(),
                "BotALot".to_string(),
                "Buddy".to_string(),
                "BuiltBotTough".to_string(),
                "Bullseye".to_string(),
                "BunnySlippers".to_string(),
                "Cegbfeieh".to_string(),
                "CheeseBot".to_string(),
                "CherryPicker".to_string(),
                "ChinaClaw".to_string(),
                "Collector".to_string(),
                "Copier".to_string(),
                "CopyRightCheck".to_string(),
                "cosmos".to_string(),
                "Crescent".to_string(),
                "Custo".to_string(),
                "AIBOT".to_string(),
                "DISCo".to_string(),
                "DIIbot".to_string(),
                "DittoSpyder".to_string(),
                "Download\\ Demon".to_string(),
                "Download\\ Ninja".to_string(),
                "eCatch".to_string(),
                "EasyDL".to_string(),
                "EirGrabber".to_string(),
                "EmailCollector".to_string(),
                "EmailSiphon".to_string(),
                "EmailWolf".to_string(),
                "EroCrawler".to_string(),
                "Express\\ WebPictures".to_string(),
                "ExtractorPro".to_string(),
                "EyeNetIE".to_string(),
                "FlashGet".to_string(),
                "FrontPage".to_string(),
                "GetRight".to_string(),
                "GetWeb!".to_string(),
                "Go!Zilla".to_string(),
                "Go-Ahead-Got-It".to_string(),
                "GrabNet".to_string(),
                "Grafula".to_string(),
                "Harvest".to_string(),
                "hloader".to_string(),
                "HMView".to_string(),
                "HTTrack".to_string(),
                "humanlinks".to_string(),
                "IlseBot".to_string(),
                "Image\\ Stripper".to_string(),
                "Image\\ Sucker".to_string(),
                "Indy\\ Library".to_string(),
                "InfoNaviRobot".to_string(),
                "InterGET".to_string(),
                "Internet\\ Ninja".to_string(),
                "Iria".to_string(),
                "Jakarta".to_string(),
                "JennyBot".to_string(),
                "JetCar".to_string(),
                "JOC\\ Web\\ Spider".to_string(),
                "Kenjin.Spider".to_string(),
                "Keyword.Density".to_string(),
                "larbin".to_string(),
                "LexiBot".to_string(),
                "libWeb/clsHTTP".to_string(),
                "LinkextractorPro".to_string(),
                "LinkScan/8.1a.Unix".to_string(),
                "LinkWalker".to_string(),
                "LNSpiderguy".to_string(),
                "lwp-trivial".to_string(),
                "LWP::Simple".to_string(),
                "Magnet".to_string(),
                "Mag-Net".to_string(),
                "MarkWatch".to_string(),
                "Mass\\ Downloader".to_string(),
                "Mata.Hari".to_string(),
                "Memo".to_string(),
                "Microsoft.URL".to_string(),
                "MIDown\\ tool".to_string(),
                "MIIxpc".to_string(),
                "Mirror".to_string(),
                "Missigua\\ Locator".to_string(),
                "Mister\\ PiX".to_string(),
                "MJ12bot".to_string(),
                "Mozilla/3.Mozilla/2.01".to_string(),
                "Mozilla.*NEWT".to_string(),
                "NAMEPROTECT".to_string(),
                "Navroad".to_string(),
                "NearSite".to_string(),
                "NetAnts".to_string(),
                "Netcraft".to_string(),
                "NetMechanic".to_string(),
                "NetSpider".to_string(),
                "Net\\ Vampire".to_string(),
                "NetZIP".to_string(),
                "NextGenSearchBot".to_string(),
                "NICErsPRO".to_string(),
                "NimbleCrawler".to_string(),
                "Ninja".to_string(),
                "NPbot".to_string(),
                "Octopus".to_string(),
                "Offline\\ Explorer".to_string(),
                "Offline\\ Navigator".to_string(),
                "Openfind".to_string(),
                "OutfoxBot".to_string(),
                "PageGrabber".to_string(),
                "Papa\\ Foto".to_string(),
                "pavuk".to_string(),
                "pcBrowser".to_string(),
                "Go-Ahead-Got-It".to_string(),
                "RealDownload".to_string(),
                "ReGet".to_string(),
                "SiteSnagger".to_string(),
                "SmartDownload".to_string(),
                "SuperBot".to_string(),
                "SuperHTTP".to_string(),
                "Surfbot".to_string(),
                "tAkeOut".to_string(),
                "Teleport\\ Pro".to_string(),
                "VoidEYE".to_string(),
                "Web\\ Image\\ Collector".to_string(),
                "Web\\ Sucker".to_string(),
                "WebAuto".to_string(),
                "WebCopier".to_string(),
                "WebFetch".to_string(),
                "WebGo\\ IS".to_string(),
                "WebLeacher".to_string(),
                "WebReaper".to_string(),
                "WebSauger".to_string(),
                "Website\\ eXtractor".to_string(),
                "Website\\ Quester".to_string(),
                "WebStripper".to_string(),
                "WebWhacker".to_string(),
                "WebZIP".to_string(),
                "Wget".to_string(),
                "Widow".to_string(),
                "WWWOFFLE".to_string(),
                "Xaldon\\ WebSpider".to_string(),
                "Zeus".to_string(),
            ],
        }
    }
}

impl NginxBadBotJail {
    pub fn new() -> Self {
        Self::default()
    }

    /// Generates the contents for the fail2ban filter definition.
    pub fn generate_filter_config(&self) -> String {
        let mut config = String::new();
        let _ = writeln!(config, "[Definition]");

        let bad_bot_pattern = self.bad_bots.join("|");

        // This is a typical regex to match bad bot user agents in nginx logs.
        let _ = writeln!(
            config,
            "failregex = ^<HOST> -.*\"(GET|POST|HEAD).*HTTP.*\" .*\"(?:{})\".*$",
            bad_bot_pattern
        );
        let _ = writeln!(config, "ignoreregex = ");

        config
    }

    /// Generates the contents for the fail2ban jail definition.
    pub fn generate_jail_config(&self) -> String {
        let mut config = String::new();
        let _ = writeln!(config, "[{}]", self.name);
        let _ = writeln!(config, "enabled = true");
        let _ = writeln!(config, "port = http,https");
        let _ = writeln!(config, "filter = {}", self.name);

        let paths = self.log_paths.join(" ");
        let _ = writeln!(config, "logpath = {}", paths);

        let _ = writeln!(config, "maxretry = {}", self.maxretry);
        let _ = writeln!(config, "findtime = {}", self.findtime);
        let _ = writeln!(config, "bantime = {}", self.bantime);

        config
    }
}
