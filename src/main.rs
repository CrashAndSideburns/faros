#[macro_use]
extern crate clap;

use chrono::{DateTime, Datelike, Timelike, Local, TimeZone};
use serde::{Serialize, Deserialize};
use serde_json;
use std::{io, env, path, fs, fmt};
use std::io::{Write};
use clap::{App, Arg, ArgMatches};
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
struct TODOList {
    tasks: Vec<Task>,
    tags: Vec<Tag>
}

#[derive(Serialize, Deserialize)]
enum Completion {
    Complete,
    Incomplete(Vec<Task>)
}

impl TODOList {
    fn read() -> TODOList {
        // Reads the TODOList from ~/.config/faros/list.json, creating anything that does not exist. If serializing the TODOList struct fails due to an unexpected EOF, we assume that the file is empty and return a new empty TODOList.
        fs::create_dir_all(path::Path::new(&env::var_os("HOME").unwrap_or_else(|| {
            eprintln!("$HOME environment variable does not exist.");
            std::process::exit(1);
        })).join(".config")
           .join("faros")).unwrap_or_else(|_| {
               eprintln!("~/.config/faros does not exist and could not be created.");
               std::process::exit(1);
           });
        serde_json::from_reader(io::BufReader::new(
            fs::OpenOptions::new().read(true).write(true).create(true).open(
                path::Path::new(&env::var_os("HOME").unwrap()).join(".config").join("faros").join("list.json"))
                .unwrap_or_else(|_| {
                    eprintln!("~/config/faros/list.json could not be opened, or could not be created if it doesn't exist.");
                    std::process::exit(1);
                }))).unwrap_or_else(|err| {
                    match err.classify() {
                        serde_json::error::Category::Eof => TODOList{tasks: Vec::new(), tags: Vec::new()},
                        _ => {
                            eprintln!("~/config/faros/list.json exists, but couldn't be parsed.");
                            std::process::exit(1);
                        }
                    }
                })
    }

    fn write(self) {
        // Write a TODOList to ~/.config/faros/list.json. This method assumes that the file already exists, and will not create it if it does not.
        serde_json::to_writer(fs::OpenOptions::new().write(true).open(
            path::Path::new(&env::var_os("HOME").unwrap()).join(".config").join("faros").join("list.json"))
            .unwrap_or_else(|_| {
                eprintln!("~/.config/faros/list.json could not be written to.");
                std::process::exit(1);
            }), 
            &self).unwrap_or_else(|_| {
                eprintln!("Your TODO list could not be serialized.");
                std::process::exit(1);
            });
    }

    fn task_from_name(&mut self, name: &str) -> Option<&mut Task> {
        // Get a mutable reference to a task in the TODO list with the given name, dealing with the fact that there may be multiple tasks with the same name. Return None if there are no tasks with the given name.
        let mut tasks = self.flattened();
        tasks.retain(|t| t.name.as_str() == name);
        let uuid = match tasks.len() {
            0 => None,
            1 => Some(tasks[0].uuid),
            n => {
                println!("There is more than one task in your TODO list named {}. Select one.", name);
                for task in &tasks {
                    println!("{}", task);
                }
                let mut buffer = String::new();
                io::stdin().read_line(&mut buffer);
                let index = buffer.trim().parse::<usize>().unwrap_or_else(|_| {
                    eprintln!("Error: Unexpected value, expected [int]. found\"{}\".", buffer.trim());
                    std::process::exit(1);
                });
                if index >= n {
                    eprintln!("Expected a value less than {}, found {}", n, index);
                    std::process::exit(1);
                }
                Some(tasks[index].uuid)
            }
        };

        uuid.map(move |u| self.task_from_uuid(u))
    }

    fn task_from_uuid(&mut self, uuid: Uuid) -> &mut Task {
        for task in &mut self.tasks {
            if let Some(valid_task) = task.task_from_uuid(uuid) {
                return valid_task;
            }
        }

        panic!("If you're seeing this, Morgan REALLY fucked up.");
    }

    fn flattened(&self) -> Vec<&Task> {
        let mut tasks = Vec::new();
        for task in &self.tasks {
            tasks.append(&mut task.flattened());
        }
        tasks
    }

    fn remove_uuid(&mut self, uuid: Uuid) {
        for task in &mut self.tasks {
            task.remove_uuid(uuid);
        }
    }
}

#[derive(Serialize, Deserialize)]
struct Task {
    name: String,
    description: String,
    priority: Priority,
    due_date: DateTime<Local>,
    completion: Completion,
    uuid: Uuid,
    tags: Vec<Uuid>

}

#[derive(Serialize, Deserialize)]
struct Tag {
    name: String,
    description: String,
    uuid: Uuid
}

#[derive(Serialize, Deserialize)]
enum Priority {
    High,
    Medium,
    Low
}

// This is a temporary, functional implementation. It still needs to be made pretty.
impl fmt::Display for Task {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Name: {}\n\tDescription: {}\n\tDue Date: {}\n\t{}", self.name, self.description, self.due_date, self.uuid)
    }
}

impl Task {
    fn new(name: String, description: String, priority:Priority, due_date: DateTime<Local>) -> Task {
        Task {
            name,
            description,
            priority,
            due_date,
            completion: Completion::Incomplete(Vec::new()),
            uuid: Uuid::new_v4(),
            tags: Vec::new()
        }
    }

    fn flattened(&self) -> Vec<&Task> {
        let mut tasks = Vec::new();
        tasks.push(self);
        if let Completion::Incomplete(children) = &self.completion {
            for child in children {
                tasks.append(&mut child.flattened());
            }
        }
        tasks
    }

    fn task_from_uuid(&mut self, uuid:Uuid) -> Option<&mut Task> {
        if self.uuid == uuid {
            Some(self)
        } else {
            match &mut self.completion {
                Completion::Complete => None,
                Completion::Incomplete(children) => {
                    for child in children {
                        if let Some(task) = child.task_from_uuid(uuid) {
                            return Some(task);
                        }
                    }
                    None
                }
            }
        }
    }

    fn complete(&mut self) {
        match &self.completion {
            Completion::Complete => {
                println!("The task named {} is already marked as complete.", self.name)
            },
            Completion::Incomplete(children) => {
                if children.iter().any(|child| matches!(child.completion, Completion::Incomplete(_))) {
                    eprintln!("The task named {} cannot be completed, as it has incomplete subtask(s).", self.name);
                    std::process::exit(1);
                } else {
                    self.completion = Completion::Complete;
                }
            }
        }
    }

    fn valid(&self, high: bool, medium: bool, low: bool, max_days: i64) -> bool {
        if high || medium || low {
            (matches!(self.priority, Priority::High) && high ||
            matches!(self.priority, Priority::Medium) && medium ||
            matches!(self.priority, Priority::Low) && low) &&
            (max_days >= (self.due_date - Local::now()).num_days())
        } else {
            self.valid(true, true, true, max_days)
        }
    }

    fn remove_uuid(&mut self, uuid: Uuid) {
        if let Completion::Incomplete(children) = &mut self.completion {
            children.retain(|task| task.uuid != uuid);
        }
    }
}
impl Tag {
    fn new(name: String, description: String) -> Tag {
        Tag {
            name,
            description,
            uuid: Uuid::new_v4()
        }
    }
}

fn cli() -> ArgMatches<'static> {
    App::new("faros")
            .author(crate_authors!())
            .version(crate_version!())
            .about("A simple CLI TODO list manager written in Rust.")
            .subcommand(App::new("list")
                                .about("Lists tasks from your TODO list.")
                                .arg(Arg::with_name("days")
                                                   .short("d")
                                                   .long("days")
                                                   .help("Lists only tasks that are due within the specified number of days.")
                                                   .takes_value(true))
                                .arg(Arg::with_name("number")
                                                   .short("n")
                                                   .long("number")
                                                   .help("Lists a maximum of the specified number of tasks.")
                                                   .takes_value(true))
                                .arg(Arg::with_name("high")
                                                   .short("H")
                                                   .long("high")
                                                   .help("Lists only tasks marked as high priority."))
                                .arg(Arg::with_name("medium")
                                                   .short("M")
                                                   .long("medium")
                                                   .help("Lists only tasks marked as medium priority."))
                                .arg(Arg::with_name("low")
                                                   .short("L")
                                                   .long("low")
                                                   .help("Lists only tasks marked as low priority."))
                                .arg(Arg::with_name("tag")
                                                   .short("t")
                                                   .long("tag")
                                                   .help("Lists only tasks marked with the specified tag.")
                                                   .takes_value(true)
                                                   .multiple(true)))
            .subcommand(App::new("complete")
                                .about("Checks tasks off as complete.")
                                .arg(Arg::with_name("task_name")
                                                   .required(true)
                                                   .multiple(true)))
            .subcommand(App::new("add")
                                .about("Adds something to your TODO list.")
                                .subcommand(App::new("subtask")
                                                    .about("Adds a subtask of an existing task to your TODO list.")
                                                    .arg(Arg::with_name("parent_name")
                                                                       .required(true))
                                                    .arg(Arg::with_name("name")
                                                                       .short("n")
                                                                       .long("name")
                                                                       .help("Specifies the name of the subtask.")
                                                                       .takes_value(true))
                                                    .arg(Arg::with_name("description")
                                                                       .short("d")
                                                                       .long("desc")
                                                                       .help("Specifies the description of the subtask.")
                                                                       .takes_value(true))
                                                    .arg(Arg::with_name("year")
                                                                       .short("Y")
                                                                       .long("year")
                                                                       .help("Specifies the year that the subtask is due.")
                                                                       .takes_value(true))
                                                    .arg(Arg::with_name("month")
                                                                       .short("M")
                                                                       .long("month")
                                                                       .help("Specifies the month that the subtask is due.")
                                                                       .takes_value(true))
                                                    .arg(Arg::with_name("day")
                                                                       .short("D")
                                                                       .long("day")
                                                                       .help("Specifies the day that the subtask is due.")
                                                                       .takes_value(true))
                                                    .arg(Arg::with_name("hour")
                                                                       .short("h")
                                                                       .long("hour")
                                                                       .help("Specifies the hour that the subtask is due.")
                                                                       .takes_value(true))
                                                    .arg(Arg::with_name("minute")
                                                                       .short("m")
                                                                       .long("minute")
                                                                       .help("Specifies the number of minutes after the hour that the subtask is due.")
                                                                        .takes_value(true))
                                                    .arg(Arg::with_name("tags")
                                                                       .short("t")
                                                                       .long("tags")
                                                                       .help("Specifies the subtask's tags.")
                                                                       .takes_value(true)
                                                                       .multiple(true)))
                                .subcommand(App::new("task")
                                                    .about("Adds a task to your TODO list.")
                                                    .arg(Arg::with_name("name")
                                                                       .short("n")
                                                                       .long("name")
                                                                       .help("Specifies the name of the task.")
                                                                       .takes_value(true))
                                                    .arg(Arg::with_name("description")
                                                                       .short("d")
                                                                       .long("desc")
                                                                       .help("Specifies the description of the task.")
                                                                       .takes_value(true))
                                                    .arg(Arg::with_name("year")
                                                                       .short("Y")
                                                                       .long("year")
                                                                       .help("Specifies the year that the task is due.")
                                                                       .takes_value(true))
                                                    .arg(Arg::with_name("month")
                                                                       .short("M")
                                                                       .long("month")
                                                                       .help("Specifies the month that the task is due.")
                                                                       .takes_value(true))
                                                    .arg(Arg::with_name("day")
                                                                       .short("D")
                                                                       .long("day")
                                                                       .help("Specifies the day that the task is due.")
                                                                       .takes_value(true))
                                                    .arg(Arg::with_name("hour")
                                                                       .short("h")
                                                                       .long("hour")
                                                                       .help("Specifies the hour that the task is due.")
                                                                       .takes_value(true))
                                                    .arg(Arg::with_name("minute")
                                                                       .short("m")
                                                                       .long("minute")
                                                                       .help("Specifies the number of minutes after the hour that the task is due.")
                                                                       .takes_value(true))
                                                    .arg(Arg::with_name("tags")
                                                                       .short("t")
                                                                       .long("tags")
                                                                       .help("Specifies the task's tags.")
                                                                       .takes_value(true)
                                                                       .multiple(true)))
                                .subcommand(App::new("tag")
                                                    .about("Adds a tag to your TODO list.")
                                                    .arg(Arg::with_name("name")
                                                                       .short("n")
                                                                       .long("name")
                                                                       .help("Specifies the tag's name.")
                                                                       .takes_value(true))
                                                    .arg(Arg::with_name("description")
                                                                       .short("d")
                                                                       .long("desc")
                                                                       .help("Specifies the tags's description.")
                                                                       .takes_value(true))))
            .subcommand(App::new("modify")
                                .about("Modifies something in your TODO list.")
                                .subcommand(App::new("task")
                                                    .about("Modifies a task in your TODO list.")
                                                    .arg(Arg::with_name("task_name")
                                                                       .required(true))
                                                    .arg(Arg::with_name("name")
                                                                       .short("n")
                                                                       .long("name")
                                                                       .help("Specifies the name of the task.")
                                                                       .takes_value(true))
                                                    .arg(Arg::with_name("description")
                                                                       .short("d")
                                                                       .long("desc")
                                                                       .help("Specifies the description of the task.")
                                                                       .takes_value(true))
                                                    .arg(Arg::with_name("year")
                                                                       .short("Y")
                                                                       .long("year")
                                                                       .help("Specifies the year that the task is due.")
                                                                       .takes_value(true))
                                                    .arg(Arg::with_name("month")
                                                                       .short("M")
                                                                       .long("month")
                                                                       .help("Specifies the month that the task is due.")
                                                                       .takes_value(true))
                                                    .arg(Arg::with_name("day")
                                                                       .short("D")
                                                                       .long("day")
                                                                       .help("Specifies the day that the task is due.")
                                                                       .takes_value(true))
                                                    .arg(Arg::with_name("hour")
                                                                       .short("h")
                                                                       .long("hour")
                                                                       .help("Specifies the hour that the task is due.")
                                                                       .takes_value(true))
                                                    .arg(Arg::with_name("minute")
                                                                       .short("m")
                                                                       .long("minute")
                                                                       .help("Specifies the number of minutes after the hour that the task is due.")
                                                                       .takes_value(true))
                                                    .arg(Arg::with_name("tags")
                                                                       .short("t")
                                                                       .long("tags")
                                                                       .help("Specifies the task's tags.")
                                                                       .takes_value(true)
                                                                       .multiple(true)))
                                .subcommand(App::new("tag")
                                                    .about("Modifies a tag in your TODO list.")
                                                    .arg(Arg::with_name("tag_name")
                                                                       .required(true))
                                                    .subcommand(App::new("tag")
                                                    .about("Adds a tag to your TODO list.")
                                                    .arg(Arg::with_name("name")
                                                                       .short("n")
                                                                       .long("name")
                                                                       .help("Specifies the tag's name.")
                                                                       .takes_value(true))
                                                    .arg(Arg::with_name("description")
                                                                       .short("d")
                                                                       .long("desc")
                                                                       .help("Specifies the tags's description.")
                                                                       .takes_value(true)))))
            .subcommand(App::new("remove")
                                .about("Removes something from your TODO list.")
                                .subcommand(App::new("task")
                                                    .about("Removes a task from your TODO list.")
                                                    .arg(Arg::with_name("task_name")
                                                                       .required(true)
                                                                       .multiple(true)))
                                .subcommand(App::new("tag")
                                                    .about("Removes a tag from your TODO list.")
                                                    .arg(Arg::with_name("tag_name")
                                                                       .required(true)
                                                                       .multiple(true))))
            .get_matches()
}

fn main() {
    let matches = cli();
    let mut todo_list = TODOList::read();

    match matches.subcommand() {
        ("list", Some(app)) => {
            let max_days = app.value_of("days")
                              .map_or_else(|| 3, 
                                           |d| {
                                               d.parse::<i64>().unwrap_or_else(|_| {
                                                   eprintln!("Error: Unexpected value, expected [int]. found\"{}\".", d);
                                                   std::process::exit(1);
                                               })
                                           });
            let max_number = app.value_of("number")
                                         .map_or_else(|| 1000000000,
                                                      |n| {
                                                          n.parse::<usize>().unwrap_or_else(|_| {
                                                              eprintln!("Error: Unexpected value, expected [int]. found\"{}\".", n);
                                                               std::process::exit(1);
                                                          })
                                                      });
            let high = app.is_present("high");
            let medium = app.is_present("medium");
            let low = app.is_present("low");
            let _tags = app.values_of("tag").and_then(|t| Some(t.collect::<Vec<_>>()));

            let mut tasks = todo_list.flattened();
            tasks.sort_by(|task1, task2| task1.due_date.cmp(&task2.due_date));
            for task in tasks.iter().filter(|t| t.valid(high, medium, low, max_days)).take(max_number) {
                println!("{}", task);
            }
        },
        ("complete", Some(app)) => {
            let task_names = app.values_of("task_name").unwrap().collect::<Vec<_>>();
            for name in task_names {
                match todo_list.task_from_name(name) {
                    Some(task) => task.complete(),
                    None => {
                        eprintln!("No task with name {}.", name);
                    }
                }
            }
        },
        ("add", Some(app)) => {
            match app.subcommand() {
                ("task", Some(subapp)) => {
                    let name = subapp.value_of("name")
                                     .map_or_else(|| {
                                         print!("Please give your new task a name: ");
                                         io::stdout().flush();
                                         let mut buffer = String::new();
                                         io::stdin().read_line(&mut buffer);
                                         String::from(buffer.trim())
                                     },
                                                  |s| String::from(s));
                    let description = subapp.value_of("description")
                                            .map_or_else(|| {
                                                print!("Please give your new task a description: ");
                                                io::stdout().flush();
                                                let mut buffer = String::new();
                                                io::stdin().read_line(&mut buffer);
                                                String::from(buffer.trim())
                                            }, 
                                                         |s| String::from(s));
                    let year = app.value_of("year")
                                  .map_or_else(|| Local::now().year(),
                                               |y| y.parse::<i32>().unwrap_or_else(|_| {
                                                   eprintln!("Error: Unexpected value, expected [int], found \"{}\".", y);
                                                   std::process::exit(1);
                                               }));
                    let month = app.value_of("month")
                                   .map_or_else(|| Local::now().month(),
                                                |m| m.parse::<u32>().unwrap_or_else(|_| {
                                                    eprintln!("Error: Unexpected value, expected [int], found \"{}\".", m);
                                                    std::process::exit(1);
                                                }));
                    let day = app.value_of("day")
                                 .map_or_else(|| Local::now().day(),
                                              |d| d.parse::<u32>().unwrap_or_else(|_| {
                                                  eprintln!("Error: Unexpected value, expected [int], found \"{}\".", d);
                                                  std::process::exit(1);
                                              }));
                    let hour = app.value_of("hour")
                                  .map_or_else(|| 23,
                                               |h| h.parse::<u32>().unwrap_or_else(|_| {
                                                   eprintln!("Error: Unexpected value, expected [int], found \"{}\".", h);
                                                   std::process::exit(1);
                                               }));
                    let minute = app.value_of("minute")
                                    .map_or_else(|| 59,
                                                 |m| m.parse::<u32>().unwrap_or_else(|_| {
                                                     eprintln!("Error: Unexpected value, expected [int], found \"{}\".", m);
                                                     std::process::exit(1);
                                                 }));

                    todo_list.tasks.push(
                        Task::new(name, description, Priority::Medium, Local.ymd(year, month, day).and_hms(hour, minute, 0)));
                },
                ("subtask", Some(subapp)) => {
                    let parent_name = subapp.value_of("parent_name").unwrap();
                    let name = subapp.value_of("name")
                                     .map_or_else(|| {
                                         print!("Please give your new task a name: ");
                                         io::stdout().flush();
                                         let mut buffer = String::new();
                                         io::stdin().read_line(&mut buffer);
                                         String::from(buffer.trim())
                                     },
                                                  |s| String::from(s));
                    let description = subapp.value_of("description")
                                            .map_or_else(|| {
                                                print!("Please give your new task a description: ");
                                                io::stdout().flush();
                                                let mut buffer = String::new();
                                                io::stdin().read_line(&mut buffer);
                                                String::from(buffer.trim())
                                            }, 
                                                         |s| String::from(s));
                    let year = app.value_of("year")
                                  .map_or_else(|| Local::now().year(),
                                               |y| y.parse::<i32>().unwrap_or_else(|_| {
                                                   eprintln!("Error: Unexpected value, expected [int], found \"{}\".", y);
                                                   std::process::exit(1);
                                               }));
                    let month = app.value_of("month")
                                   .map_or_else(|| Local::now().month(),
                                                |m| m.parse::<u32>().unwrap_or_else(|_| {
                                                    eprintln!("Error: Unexpected value, expected [int], found \"{}\".", m);
                                                    std::process::exit(1);
                                                }));
                    let day = app.value_of("day")
                                 .map_or_else(|| Local::now().day(),
                                              |d| d.parse::<u32>().unwrap_or_else(|_| {
                                                  eprintln!("Error: Unexpected value, expected [int], found \"{}\".", d);
                                                  std::process::exit(1);
                                              }));
                    let hour = app.value_of("hour")
                                  .map_or_else(|| 23,
                                               |h| h.parse::<u32>().unwrap_or_else(|_| {
                                                   eprintln!("Error: Unexpected value, expected [int], found \"{}\".", h);
                                                   std::process::exit(1);
                                               }));
                    let minute = app.value_of("minute")
                                    .map_or_else(|| 59,
                                                 |m| m.parse::<u32>().unwrap_or_else(|_| {
                                                     eprintln!("Error: Unexpected value, expected [int], found \"{}\".", m);
                                                     std::process::exit(1);
                                                 }));

                    match &mut todo_list.task_from_name(parent_name).unwrap_or_else(|| {
                        eprintln!("There is no task with name {} to which a subtask can be added.", parent_name);
                        std::process::exit(1);
                    }).completion {
                        Completion::Complete => {
                            todo_list.task_from_name(parent_name)
                                .unwrap()
                                .completion = Completion::Incomplete(
                                    vec![Task::new(name, 
                                        description, 
                                        Priority::Medium, 
                                        Local.ymd(year, month, day).and_hms(hour, minute, 0))]);
                        },
                        Completion::Incomplete(children) => {
                            children.push(Task::new(name, 
                                description, 
                                Priority::Medium, 
                                Local.ymd(year, month, day).and_hms(hour, minute, 0)));
                        }
                    }
                },
                ("tag", Some(subapp)) => {
                    let _name = subapp.value_of("name")
                                     .map_or_else(|| {
                                         print!("Please give your new tag a name: ");
                                         io::stdout().flush();
                                         let mut buffer = String::new();
                                         io::stdin().read_line(&mut buffer);
                                         String::from(buffer.trim())
                                     },
                                                  |s| String::from(s));
                    let _description = subapp.value_of("description")
                                            .map_or_else(|| {
                                                print!("Please give your new tag a description: ");
                                                io::stdout().flush();
                                                let mut buffer = String::new();
                                                io::stdin().read_line(&mut buffer);
                                                String::from(buffer.trim())
                                            },
                                                         |s| String::from(s));
                    //TODO: Adding tags and managing them will probably be a future version thing.
                },
                _ => ()
            }
        },
        ("modify", Some(app)) => {
            match app.subcommand() {
                ("task", Some(subapp)) => {
                    let task_name = subapp.value_of("task_name").unwrap();
                    let name = subapp.value_of("name");
                    let year = subapp.value_of("year").map(|y| {
                        y.parse::<i32>().unwrap_or_else(|_| {
                            eprintln!("Error: Unexpected value, expected [int], found \"{}\".", y);
                            std::process::exit(1);
                        })
                    });
                    let month = subapp.value_of("month").map(|m| {
                        m.parse::<u32>().unwrap_or_else(|_| {
                            eprintln!("Error: Unexpected value, expected [int], found \"{}\".", m);
                            std::process::exit(1);
                        })
                    });
                    let day = subapp.value_of("day").map(|d| {
                        d.parse::<u32>().unwrap_or_else(|_| {
                            eprintln!("Error: Unexpected value, expected [int], found \"{}\".", d);
                            std::process::exit(1);
                        })
                    });
                    let hour = subapp.value_of("hour").map(|h| {
                        h.parse::<u32>().unwrap_or_else(|_| {
                            eprintln!("Error: Unexpected value, expected [int], found \"{}\".", h);
                            std::process::exit(1);
                        })
                    });
                    let minute = subapp.value_of("minute").map(|m| {
                        m.parse::<u32>().unwrap_or_else(|_| {
                            eprintln!("Error: Unexpected value, expected [int], found \"{}\".", m);
                            std::process::exit(1);
                        })
                    });
                    let _tags = subapp.values_of("tag").and_then(|t| Some(t.collect::<Vec<_>>()));

                    let task = todo_list.task_from_name(task_name).unwrap_or_else(|| {
                        eprintln!("There is no task named {}", task_name);
                        std::process::exit(1);
                    });

                    if let Some(n) = name {
                        task.name = String::from(n);
                    }
                    if let Some(y) = year {
                        task.due_date = Local.ymd(y, 
                            task.due_date.month(), 
                            task.due_date.day())
                            .and_hms(
                                task.due_date.hour(), 
                                task.due_date.minute(), 
                                0);
                    }
                    if let Some(m) = month {
                        task.due_date = Local.ymd(task.due_date.year(), 
                            m, 
                            task.due_date.day())
                            .and_hms(
                                task.due_date.hour(), 
                                task.due_date.minute(), 
                                0);
                    }
                    if let Some(d) = day {
                        task.due_date = Local.ymd(task.due_date.year(), 
                            task.due_date.month(), 
                            d)
                            .and_hms(
                                task.due_date.hour(), 
                                task.due_date.minute(), 
                                0);
                    }
                    if let Some(h) = hour {
                        task.due_date = Local.ymd(task.due_date.year(), 
                            task.due_date.month(), 
                            task.due_date.day())
                            .and_hms(
                                h, 
                                task.due_date.minute(), 
                                0);
                    }
                    if let Some(m) = minute {
                        task.due_date = Local.ymd(task.due_date.year(), 
                            task.due_date.month(), 
                            task.due_date.day())
                            .and_hms(
                                task.due_date.hour(), 
                                m, 
                                0);
                    }
                },
                ("tag", Some(subapp)) => {
                    let _tag_name = subapp.value_of("tag_name").unwrap();
                    let _name = subapp.value_of("name");
                    let _description = subapp.value_of("description");
                    // This should be implemented at the same time that adding tags is.
                },
                _ => ()
            }
        },
        ("remove", Some(app)) => {
            match app.subcommand() {
                ("task", Some(subapp)) => {
                    let task_names = subapp.values_of("task_name").unwrap().collect::<Vec<_>>();
                    for name in task_names {
                        let task_uuid = todo_list.task_from_name(name).unwrap_or_else(|| {
                            eprintln!("There is no task named {}", name);
                            std::process::exit(1);
                        }).uuid;
                        todo_list.remove_uuid(task_uuid);
                    }
                },
                ("tag", Some(subapp)) => {
                    let _tag_names = subapp.values_of("task_name").unwrap().collect::<Vec<_>>();
                    // Again, to be implemented at the same time as task addition and modification.
                },
                _ => ()
            }
        },
        _ => ()
    }

    todo_list.write();
}
