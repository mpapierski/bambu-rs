use std::str::FromStr;

use chrono::{Datelike, NaiveDateTime};

#[derive(Debug, PartialEq, Eq)]
pub struct Permissions {
    pub directory: bool,
    pub readable: bool,
    pub writable: bool,
    pub executable: bool,
}

impl Permissions {
    fn from_chmod(chmod: &str) -> Result<Self, String> {
        if chmod.len() != 10 {
            return Err("Invalid chmod format".to_string());
        }

        let directory = chmod.chars().nth(0) == Some('d');
        let readable = chmod.chars().nth(1) == Some('r')
            && chmod.chars().nth(4) == Some('r')
            && chmod.chars().nth(7) == Some('r');
        let writable = chmod.chars().nth(2) == Some('w')
            && chmod.chars().nth(5) == Some('w')
            && chmod.chars().nth(8) == Some('w');
        let executable = chmod.chars().nth(3) == Some('x')
            && chmod.chars().nth(6) == Some('x')
            && chmod.chars().nth(9) == Some('x');

        Ok(Permissions {
            directory,
            readable,
            writable,
            executable,
        })
    }

    pub fn to_octal(&self) -> u16 {
        let mut mode = 0;
        if self.readable {
            mode |= 0o444;
        }
        if self.writable {
            mode |= 0o222;
        }
        if self.executable {
            mode |= 0o111;
        }
        mode
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct FileMetadata {
    pub chmod: Permissions,
    pub user: String,
    pub group: String,
    pub size: u64,
    pub date: NaiveDateTime,
    pub filename: String,
}

impl FromStr for FileMetadata {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split_whitespace().collect();

        if parts.len() < 9 {
            return Err("Invalid FTP LIST line".to_string());
        }

        // Extract chmod, owner, size, and filename
        let chmod = Permissions::from_chmod(parts[0])?;
        let user = parts[2].to_string();
        let group = parts[3].to_string();

        let size: u64 = parts[4]
            .parse()
            .map_err(|_| "Invalid size format".to_string())?;

        // Extract date and time
        let month = parts[5];
        let day: u32 = parts[6]
            .parse()
            .map_err(|_| "Invalid day format".to_string())?;
        let year_or_time = parts[7];

        let date = if year_or_time.contains(":") {
            let current_year = chrono::Local::now().year();
            let datetime_str = format!("{} {} {} {}", current_year, month, day, year_or_time);
            NaiveDateTime::parse_from_str(&datetime_str, "%Y %b %d %H:%M")
                .map_err(|_| "Invalid time format".to_string())?
        } else {
            let datetime_str = format!("{} {} {} 00:00", year_or_time, month, day);
            NaiveDateTime::parse_from_str(&datetime_str, "%Y %b %d %H:%M")
                .map_err(|_| "Invalid date format".to_string())?
        };

        // Extract filename
        let filename = parts[8..].join(" ");

        Ok(FileMetadata {
            chmod,
            user,
            group,
            size,
            date,
            filename,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use chrono::Datelike;

    use crate::file::ftp::metadata::{FileMetadata, Permissions};

    const TESTVEC1: &str = "drw-rw-rw-   1 usr1  grp1           0 Jan 01 1980 foo bar";
    const TESTVEC2: &str = "-rw-rw-rw-   1 user2 grp2   912934592 Jan 23 01:27 3D Benchy.gcode.3mf";
    const TESTVEC3: &str =
        "-rw-rw-rw-   1 root  root   124213455 Jul 23 2024 Foo Bar Baz Bar.gcode.3mf";
    #[test]
    fn test_parse_file_metadata() {
        assert_eq!(
            FileMetadata::from_str(TESTVEC1).unwrap(),
            FileMetadata {
                chmod: Permissions {
                    directory: true,
                    readable: true,
                    writable: true,
                    executable: false
                },
                user: "usr1".to_string(),
                group: "grp1".to_string(),
                size: 0,
                date: chrono::NaiveDate::from_ymd_opt(1980, 1, 1)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap(),
                filename: "foo bar".to_string()
            }
        );

        assert_eq!(
            FileMetadata::from_str(TESTVEC2).unwrap(),
            FileMetadata {
                chmod: Permissions {
                    directory: false,
                    readable: true,
                    writable: true,
                    executable: false
                },
                user: "user2".to_string(),
                group: "grp2".to_string(),
                size: 912934592,
                date: chrono::NaiveDate::from_ymd_opt(chrono::Local::now().year(), 1, 23)
                    .unwrap()
                    .and_hms_opt(1, 27, 0)
                    .unwrap(),
                filename: "3D Benchy.gcode.3mf".to_string()
            }
        );

        assert_eq!(
            FileMetadata::from_str(TESTVEC3).unwrap(),
            FileMetadata {
                chmod: Permissions {
                    directory: false,
                    readable: true,
                    writable: true,
                    executable: false
                },
                user: "root".to_string(),
                group: "root".to_string(),
                size: 124213455,
                date: chrono::NaiveDate::from_ymd_opt(2024, 7, 23)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap(),
                filename: "Foo Bar Baz Bar.gcode.3mf".to_string()
            }
        );
    }
}
