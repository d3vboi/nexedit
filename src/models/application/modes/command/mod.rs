mod displayable_command;

pub use self::displayable_command::DisplayableCommand;
use crate::commands::{self, Command};
use crate::models::application::modes::{SearchSelectConfig, SearchSelectMode};
use crate::util::SelectableVec;
use fragment;
use std::collections::HashMap;
use std::fmt;
use std::slice::Iter;

pub struct CommandMode {
    insert: bool,
    input: String,
    commands: HashMap<&'static str, Command>,
    results: SelectableVec<DisplayableCommand>,
    config: SearchSelectConfig,
}

impl CommandMode {
    pub fn new(config: SearchSelectConfig) -> CommandMode {
        CommandMode {
            insert: true,
            input: String::new(),
            commands: commands::hash_map(),
            results: SelectableVec::new(Vec::new()),
            config,
        }
    }
}

impl fmt::Display for CommandMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "COMMAND")
    }
}

impl SearchSelectMode<DisplayableCommand> for CommandMode {
    fn search(&mut self) {
        let commands: Vec<&'static str> = self.commands.keys().copied().collect();

        let results = fragment::matching::find(&self.input, &commands, self.config.max_results);

        self.results = SelectableVec::new(
            results
                .into_iter()
                .filter_map(|result| {
                    self.commands
                        .get(*result)
                        .map(|command| DisplayableCommand {
                            description: *result,
                            command: *command,
                        })
                })
                .collect(),
        );
    }

    fn query(&mut self) -> &mut String {
        &mut self.input
    }

    fn insert_mode(&self) -> bool {
        self.insert
    }

    fn set_insert_mode(&mut self, insert_mode: bool) {
        self.insert = insert_mode;
    }

    fn results(&self) -> Iter<DisplayableCommand> {
        self.results.iter()
    }

    fn selection(&self) -> Option<&DisplayableCommand> {
        self.results.selection()
    }

    fn selected_index(&self) -> usize {
        self.results.selected_index()
    }

    fn select_previous(&mut self) {
        self.results.select_previous();
    }

    fn select_next(&mut self) {
        self.results.select_next();
    }

    fn config(&self) -> &SearchSelectConfig {
        &self.config
    }
}
