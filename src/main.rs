use rayon::prelude::*;
use std::io::{Write, stdout};
use std::time::{Duration, Instant};
use crossterm::{
    ExecutableCommand, QueueableCommand,
    queue,
    terminal,
    event::{Event, KeyEvent, KeyCode, KeyModifiers, read, poll},
    cursor,
    style::Print
};


//// Logic FNs ////

// creates a new grid of x/y size optionally taking extra data from another grid
fn gen_grid(cols: usize, rows: usize, grid: Option<Vec<Vec<bool>>>) ->  Vec<Vec<bool>> {
    match grid {
        Some(mut data) => {
            for col in &mut data{
                col.resize(cols, false)
            }
            data.resize(rows, vec![false; cols]);
            data
        }
        None => vec![vec![false; cols]; rows],
    }
}


// Toggles a point on the grid between true and false
fn grid_toggle(grid: &mut Vec<Vec<bool>>, col: usize, row: usize) {
    grid[row][col] = !grid[row][col]
}


// Returns a grid advanced one step in the GOL
fn gol_step(grid: &Vec<Vec<bool>>, live: i32, birth: i32) -> Vec<Vec<bool>> {
    // cast to i32's so subtractions don't panic.
    // Unfortunately means recasting as usize later. Doesn't matter since get() bounds checks,
    // and I strongly doubt someone has a screen size of a few billion tiles.
    let max_x = grid[0].len() as i32;
    let max_y = grid.len() as i32;

    // returns a Vec<Vec<bool>>
    (0..max_y).into_par_iter().map(|y| {
        (0..max_x).into_par_iter().map(|x| {
            let mut neighbors = 0;
            // list of possible neighbors
            let coords = [
                [x, y - 1],  // up
                [x + 1, y - 1],  // up right
                [x + 1, y],  // right
                [x + 1, y + 1],  // right down
                [x, y + 1],  // down
                [x - 1, y + 1],  // down left
                [x - 1, y],  // left
                [x - 1, y - 1],  // left up
            ];

            for point in coords.iter() {
                // if the value underflows back to usize::max,
                // it'll be out-of-bounds anyway
                match grid.get(point[1] as usize) {
                    Some(row) => {
                        match row.get(point[0] as usize) {
                            // Can't compare &true to true apparently.
                            Some(val) => if val == &true {neighbors += 1;}
                            None => (),
                        }
                    },
                    None => (),
                }
            }

            // actual GOL logic
            if neighbors == birth {
                true
            } else if neighbors >= live && neighbors < birth && grid[y as usize][x as usize] {
                true
            } else {false}
        }).collect()
    }).collect()
}


//// UI FNs ////

// creates the string for the toolbar.
fn gen_toolbar<I, F>(fg_char: char, bg_char: char, live: I, birth: I, framerate: F) -> String where
    I: std::fmt::Display,
    F: std::fmt::Display,
{
    format!("FG:'{}' BG:'{}' Live:{} Birth:{} FPS:{:.1}", fg_char, bg_char, live, birth, framerate)
}


// returns the grid as a long string. ncurses should wrap, so newlines aren't added
fn grid_to_str(grid: &Vec<Vec<bool>>, char_true: char, char_false: char) -> String {
    let mut result = String::new();
    for row in grid{
        for col in row {
            match col {
                true => result.push(char_true),
                false => result.push(char_false),
            }
        }
    }
    result
}


// clears terminal and redraws text.
fn redraw<T: Write>(buff: &mut T, text: &str) {
    buff.queue(cursor::SavePosition).unwrap();
    let mut i = 0;
    for slice in text.split('\n') {
        buff.queue(cursor::MoveTo(0, i)).unwrap()
            .queue(Print(slice)).unwrap();
        i += 1;
    }
    buff.queue(cursor::RestorePosition)
        .unwrap()
        .flush()
        .unwrap();
}


// returns true if char is an acceptable display character
fn valid_chars(c: char) -> bool{
    c.is_alphanumeric() || c.is_whitespace() || c.is_ascii_punctuation()
}


// get crossterm event with optional poll duration.
fn get_event(duration: Option<Duration>) -> Option<Event>{
    match duration {
        Some(delay) => {
            if poll(delay).unwrap() {
                Some(read().unwrap())
            } else {None}
        },
        None => Some(read().unwrap())
    }
}


const HELP_TEXT: &str =
"Controls:
wasd  : move
space : toggle gridpoint
e     : frame advance
f     : playback
xx    : clear
qq    : quit
h     : show/hide this help

Game of Life rules:
minus/equals '-=' : adjust 'lives' rule
brackets '[]'     : adjust 'birth' rule

System settings:
comma/period ',.' : adjust max framerate
c                 : change dispaly characters";


// key event shorthand. Can match get_event to KE!(char)
macro_rules! KE {
    ($ch:expr) => {
        Event::Key(KeyEvent{code: KeyCode::Char($ch), modifiers: _})
    };
    ($ch:expr, $mod:expr) => {
        Event::Key(KeyEvent{code: KeyCode::Char($ch), modifiers: $mod})
    }
}


fn main() {
    terminal::enable_raw_mode().unwrap();
    let (mut cols, mut rows) = terminal::size().unwrap();
    let mut stdo = stdout();

    queue!(
        stdo,
        terminal::EnterAlternateScreen,
        cursor::MoveTo(cols/2, rows/2),
        ).unwrap();

    let log: bool;
    match std::env::args().find(|x| x == "-l") {
        Some(_) => log = true,
        None => log = false,
    }

    let mut ch_t = 'O';
    let mut ch_f = ' ';
    let mut live: i32 = 2;
    let mut birth: i32 = 3;
    let framerates = [0.5, 1., 2., 5., 10., 15., 20., 30., 45., 60., 90., 120., 999.];
    let mut framerate = 5;

    let mut matrix = gen_grid(cols as usize, rows as usize - 1, None);

    let mut draw_times = Vec::<u128>::new();
    let mut step_times = Vec::<u128>::new();

    // advance the game one iter
    macro_rules! step {
        () => {
            matrix = gol_step(&matrix, live, birth);
        }
    }

    // redraw the game and toolbar
    macro_rules! redraw_all {
        () => {
            redraw(&mut stdo,
                   &(grid_to_str(&matrix, ch_t, ch_f) +
                   &gen_toolbar(ch_t, ch_f, live, birth, framerates[framerate])));
        }
    }

    // blank the screen
    macro_rules! erase {
        () => {
            let blank = String::from(" ").repeat((cols*rows).into());
            redraw(&mut stdo, &blank);
        }
    }

    // erase!(), write HELP_TEXT, wait for keycode 'h', redraw_all!()
    macro_rules! show_help {
        () => {
            stdo.queue(cursor::Hide).unwrap();
            erase!();
            redraw(&mut stdo, HELP_TEXT);
            loop {
                match get_event(None) {
                    Some(KE!('h')) => break,
                    _ => (),
                }
            }
            stdo.queue(cursor::Show).unwrap();
            redraw_all!();
        }
    }

    // start off with control screen since there's no cmd args besides -l
    show_help!();

    // main loop
    loop {
        let (cur_col, cur_row) = cursor::position().unwrap();

        match get_event(None) {
            // movement
            Some(KE!('w')) => {stdo.queue(cursor::MoveUp(1)).unwrap();},
            Some(KE!('a')) => {stdo.queue(cursor::MoveLeft(1)).unwrap();},
            Some(KE!('s')) => {
                // prevent selecting the toolbar which is invalid matrix bounds
                if cur_row < rows - 2 {stdo.queue(cursor::MoveDown(1)).unwrap();}
            },
            Some(KE!('d')) => {stdo.queue(cursor::MoveRight(1)).unwrap();},

            // toggle point
            Some(KE!(' ')) => {
                grid_toggle(&mut matrix, cur_col as usize, cur_row as usize);
                redraw_all!();
            },

            // change rules
            Some(KE!('-')) => {
                live = (live-1).max(0);
                redraw_all!();
            },
            Some(KE!('=')) => {
                live = (live+1).min(9);
                redraw_all!();
            },
            Some(KE!('[')) => {
                birth = (birth-1).max(0);
                redraw_all!();
            },
            Some(KE!(']')) => {
                birth = (birth+1).min(9);
                redraw_all!();
            },

            // frame-advance
            Some(KE!('e')) =>  {
                step!();
                redraw_all!();
            }

            // change framerate
            Some(KE!(',')) => {
                framerate = (framerate as i32-1).max(0) as usize;
                redraw_all!();
            }
            Some(KE!('.')) => {
                framerate = (framerate+1).min(framerates.len()-1);
                redraw_all!();
            }

            // play. also logs performance if -l passed.
            Some(KE!('f')) =>  {
                stdo.queue(cursor::Hide).unwrap();
                let min_delay = Duration::from_micros(1);
                let max_delay = Duration::from_secs_f64(1./framerates[framerate]);
                let mut poll_time = min_delay;
                let mut delta: Duration;
                while get_event(Some(poll_time)) != Some(Event::Key(
                    KeyEvent{
                        code: KeyCode::Char('f'),
                        modifiers: KeyModifiers::empty()
                    }))
                {
                    let total_timer = Instant::now();
                    if log {
                        let step_timer = Instant::now();
                        step!();
                        step_times.push(step_timer.elapsed().as_micros());
                        let draw_timer = Instant::now();
                        redraw_all!();
                        draw_times.push(draw_timer.elapsed().as_micros());
                    } else {
                        step!();
                        redraw_all!();
                    }
                    delta = total_timer.elapsed();
                    poll_time = if max_delay > delta {max_delay - delta}
                                else {min_delay}
                }
                stdo.queue(cursor::Show).unwrap();
            }

            // clear
            Some(KE!('x')) => {
                match get_event(None) {
                    Some(KE!('x')) => {
                        matrix = gen_grid(cols as usize, rows as usize - 1, None);
                        redraw_all!();
                    },
                    _ => (),
                }
            }

            // change chars
            Some(KE!('c')) => {
                stdo.execute(cursor::MoveTo(4, rows-1)).unwrap();
                match get_event(None) {
                    Some(Event::Key(KeyEvent{code: key, modifiers: _})) => match key {
                        KeyCode::Char(c) => if valid_chars(c) && c != ch_f {ch_t = c;},
                        _ => (),
                    }
                    _ => (),
                }
                redraw_all!();
                stdo.execute(cursor::MoveTo(11, rows-1)).unwrap();
                match get_event(None) {
                    Some(Event::Key(KeyEvent{code: key, modifiers: _})) => match key {
                        KeyCode::Char(c) => if valid_chars(c) && c != ch_t {ch_f = c;},
                        _ => (),
                    }
                    _ => (),
                }
                stdo.execute(cursor::MoveTo(cur_col, cur_row)).unwrap();
                redraw_all!();
            }

            // show/hide help.
            Some(KE!('h')) => {
                show_help!();
            }

            // TODO resize. Seems like it's missing a lot of resize fns in the docs.
            // Blocking issue for 1.0
            // Some(Input::KeyResize) => {
            //     window.clear();
            //     window.refresh();
            //     // window.delwin();
            //     pancurses::resize_term(0, 0);
            //     window = pancurses::newwin(0, 0, 0, 0);

            //     cols = window.get_max_y();
            //     rows = window.get_max_x();
            //     matrix = gen_grid(rows as usize, cols as usize - 1, Some(matrix));
            //     redraw_all!();
            // }

            // quit
            Some(KE!('q')) => match get_event(None) {
                Some(KE!('q')) => break,
                _ => (),
            },

            _ => (),
        }
    }

    // cleanup
    stdo.queue(terminal::LeaveAlternateScreen).unwrap();
    terminal::disable_raw_mode().unwrap();

    if log {
        step_times.sort();
        draw_times.sort();
        println!("Step Median:\n{}\n\nDraw Median:\n{}", step_times[step_times.len()/2], draw_times[draw_times.len()/2]);
    }
}
