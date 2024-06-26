use crate::commands::{self, Result};
use crate::errors::*;
use crate::input::Key;
use crate::models::application::{Application, Mode};

pub fn move_to_previous_result(app: &mut Application) -> Result {
    if let Mode::Search(ref mut mode) = app.mode {
        mode.results
            .as_mut()
            .ok_or(NO_SEARCH_RESULTS)?
            .select_previous();
    } else {
        bail!("Can't move to search result outside of search mode");
    }

    commands::view::scroll_cursor_to_center(app).chain_err(|| SCROLL_TO_CURSOR_FAILED)?;
    move_to_current_result(app)
}

pub fn move_to_next_result(app: &mut Application) -> Result {
    if let Mode::Search(ref mut mode) = app.mode {
        mode.results
            .as_mut()
            .ok_or(NO_SEARCH_RESULTS)?
            .select_next();
    } else {
        bail!("Can't move to search result outside of search mode");
    }

    commands::view::scroll_cursor_to_center(app).chain_err(|| SCROLL_TO_CURSOR_FAILED)?;
    move_to_current_result(app)
}

pub fn move_to_current_result(app: &mut Application) -> Result {
    if let Mode::Search(ref mut mode) = app.mode {
        let buffer = app
            .workspace
            .current_buffer
            .as_mut()
            .ok_or(BUFFER_MISSING)?;
        let query = mode.input.as_ref().ok_or(SEARCH_QUERY_MISSING)?;
        let result = mode
            .results
            .as_mut()
            .ok_or(NO_SEARCH_RESULTS)?
            .selection()
            .ok_or_else(|| format!("No matches found for \"{}\"", query))?;
        buffer.cursor.move_to(result.start());
    } else {
        bail!("Can't move to search result outside of search mode");
    }

    commands::view::scroll_cursor_to_center(app).chain_err(|| SCROLL_TO_CURSOR_FAILED)?;

    Ok(())
}

pub fn accept_query(app: &mut Application) -> Result {
    if let Mode::Search(ref mut mode) = app.mode {
        mode.insert = false;
    } else {
        bail!("Can't accept search query outside of search mode");
    }
    run(app)?;

    Ok(())
}

pub fn clear_query(app: &mut Application) -> Result {
    if let Mode::Search(ref mut mode) = app.mode {
        mode.input = None;
        app.search_query = None;
    } else {
        bail!("Can't clear search outside of search mode");
    };

    Ok(())
}

pub fn push_search_char(app: &mut Application) -> Result {
    let key = app
        .view
        .last_key()
        .as_ref()
        .ok_or("View hasn't tracked a key press")?;

    if let Key::Char(c) = *key {
        if let Mode::Search(ref mut mode) = app.mode {
            let query = mode.input.get_or_insert(String::new());
            query.push(c);
            app.search_query = Some(query.clone());
        } else {
            bail!("Can't push search character outside of search mode");
        }
    } else {
        bail!("Last key press wasn't a character")
    }

    Ok(())
}

pub fn pop_search_char(app: &mut Application) -> Result {
    if let Mode::Search(ref mut mode) = app.mode {
        let query = mode.input.as_mut().ok_or(SEARCH_QUERY_MISSING)?;

        query.pop();
        app.search_query = Some(query.clone());
    } else {
        bail!("Can't pop search character outside of search mode");
    };

    Ok(())
}

pub fn run(app: &mut Application) -> Result {
    if let Mode::Search(ref mut mode) = app.mode {
        let buffer = app
            .workspace
            .current_buffer
            .as_ref()
            .ok_or(BUFFER_MISSING)?;
        mode.search(buffer)?;
    } else {
        bail!("Can't run search outside of search mode");
    }

    select_closest_result(app)?;
    move_to_current_result(app)
}

fn select_closest_result(app: &mut Application) -> Result {
    if let Mode::Search(ref mut mode) = app.mode {
        let buffer = app
            .workspace
            .current_buffer
            .as_ref()
            .ok_or(BUFFER_MISSING)?;
        let results = mode.results.as_mut().ok_or(NO_SEARCH_RESULTS)?;

        let skip_count = results
            .iter()
            .filter(|r| r.start() <= *buffer.cursor)
            .count();
        for _ in 0..skip_count {
            results.select_next();
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::commands;
    use crate::models::application::Mode;
    use crate::models::Application;
    use scribe::buffer::Position;
    use scribe::Buffer;

    #[test]
    fn move_to_previous_result_moves_cursor_to_previous_result() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("nexedit");

        buffer.cursor.move_to(Position { line: 1, offset: 3 });
        app.workspace.add_buffer(buffer);

        commands::application::switch_to_search_mode(&mut app).unwrap();
        if let Mode::Search(ref mut mode) = app.mode {
            mode.input = Some(String::from("ed"));
        }
        commands::search::accept_query(&mut app).unwrap();

        commands::search::move_to_previous_result(&mut app).unwrap();

        assert_eq!(
            *app.workspace.current_buffer.as_ref().unwrap().cursor,
            Position { line: 1, offset: 0 }
        );
    }

    #[test]
    fn move_to_previous_result_wraps_to_the_end_of_the_document() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("nexedit");
        app.workspace.add_buffer(buffer);

        commands::application::switch_to_search_mode(&mut app).unwrap();
        if let Mode::Search(ref mut mode) = app.mode {
            mode.input = Some(String::from("ed"));
        }
        commands::search::accept_query(&mut app).unwrap();

        commands::search::move_to_previous_result(&mut app).unwrap();

        assert_eq!(
            *app.workspace.current_buffer.as_ref().unwrap().cursor,
            Position { line: 2, offset: 0 }
        );
    }

    #[test]
    fn move_to_next_result_moves_cursor_to_next_result() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("nexedit");
        app.workspace.add_buffer(buffer);

        commands::application::switch_to_search_mode(&mut app).unwrap();
        if let Mode::Search(ref mut mode) = app.mode {
            mode.input = Some(String::from("ed"));
        }
        commands::search::accept_query(&mut app).unwrap();

        commands::search::move_to_next_result(&mut app).unwrap();

        assert_eq!(
            *app.workspace.current_buffer.as_ref().unwrap().cursor,
            Position { line: 1, offset: 0 }
        );
    }

    #[test]
    fn move_to_next_result_wraps_to_the_start_of_the_document() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("nexedit");

        buffer.cursor.move_to(Position { line: 1, offset: 3 });
        app.workspace.add_buffer(buffer);

        commands::application::switch_to_search_mode(&mut app).unwrap();
        if let Mode::Search(ref mut mode) = app.mode {
            mode.input = Some(String::from("ed"));
        }
        commands::search::accept_query(&mut app).unwrap();

        commands::search::move_to_next_result(&mut app).unwrap();

        assert_eq!(
            *app.workspace.current_buffer.as_ref().unwrap().cursor,
            Position { line: 0, offset: 4 }
        );
    }

    #[test]
    fn accept_query_disables_insert_sub_mode_and_moves_to_next_match() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("nexedit");

        buffer.cursor.move_to(Position { line: 0, offset: 4 });
        app.workspace.add_buffer(buffer);

        app.search_query = Some(String::from("ed"));
        commands::application::switch_to_search_mode(&mut app).unwrap();
        commands::search::accept_query(&mut app).unwrap();

        assert!(match app.mode {
            crate::models::application::Mode::Search(ref mode) => !mode.insert_mode(),
            _ => false,
        });

        assert_eq!(app.search_query, Some("ed".to_string()));

        assert_eq!(
            *app.workspace.current_buffer.as_ref().unwrap().cursor,
            Position { line: 1, offset: 0 }
        );
    }
}
