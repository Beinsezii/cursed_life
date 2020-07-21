use rayon::prelude::*;
use std::io::{Write, stdout};
use std::time::{Duration, Instant};
use crossterm::{
    ExecutableCommand, QueueableCommand,
    queue,
    cursor,
    event::{Event, KeyEvent, KeyCode, read, poll},
    style::Print,
    terminal,
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
fn redraw<T: Write>(buff: &mut T, mut text: String, bounds: Option<[u16;2]>) {
    buff.queue(cursor::SavePosition).unwrap();
    let mut i = 0;
    match bounds {
        Some(cols_rows) => {
            let max = (cols_rows[0] * cols_rows[1]) as usize;
            if text.len() > max {
                text.truncate(max);
            }
        },
        None => (),
    };
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


//// Standalone macros ////

// key event shorthand. Can match get_event to KE!(char)
macro_rules! KE {
    ($ch:expr) => {
        Event::Key(KeyEvent{code: KeyCode::Char($ch), modifiers: _})
    };
    ($ch:expr, $mod:expr) => {
        Event::Key(KeyEvent{code: KeyCode::Char($ch), modifiers: $mod})
    };
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
c                 : change characters

Command flags:
-l : log performance stats
-h : print this help and exit";


fn main() {
    // -h flag
    match std::env::args().find(|x| x == "-h") {
        Some(_) => {
            println!("{}", HELP_TEXT);
            return
        },
        None => (),
    }

    // -l flag
    let log: bool;
    match std::env::args().find(|x| x == "-l") {
        Some(_) => log = true,
        None => log = false,
    }

    // initializations
    terminal::enable_raw_mode().unwrap();
    let (mut cols, mut rows) = terminal::size().unwrap();
    let mut stdo = stdout();

    queue!(
        stdo,
        terminal::EnterAlternateScreen,
        cursor::MoveTo(cols/2, rows/2),
        cursor::DisableBlinking,
        ).unwrap();

    // game data
    let mut ch_t = 'O';
    let mut ch_f = ' ';
    let mut live: i32 = 2;
    let mut birth: i32 = 3;
    let framerates = [0.5, 1., 2., 5., 10., 15., 20., 30., 45., 60., 90., 120., 999.];
    let mut framerate = 5; // 15.

    let mut matrix = gen_grid(cols as usize, rows as usize - 1, None);

    let mut draw_times = Vec::<u128>::new();
    let mut step_times = Vec::<u128>::new();
    let mut framerate_averages = Vec::<f64>::new();

    //// Macros that use game data ////

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
                grid_to_str(&matrix, ch_t, ch_f) +
                &gen_toolbar(ch_t, ch_f, live, birth, framerates[framerate]),
                Some([cols, rows]));
        }
    }

    // blank the screen
    macro_rules! erase {
        () => {
            let blank = String::from(" ").repeat((cols*rows).into());
            redraw(&mut stdo, blank, None);
        }
    }

    // update cols rows, resize grid, erase!() and redraw_all!().
    macro_rules! resize {
        () => {
            let (new_cols, new_rows) = terminal::size().unwrap();
            resize!(new_cols, new_rows);
            };
        ($new_cols: expr, $new_rows: expr) => {
            cols = $new_cols;
            rows = $new_rows;
            matrix = gen_grid(cols as usize, rows as usize - 1, Some(matrix));
            // if you  don't erase chars can get left over in lower-right corner.
            erase!();
            redraw_all!();
            }
    }

    // erase!(), write HELP_TEXT, wait for keycode 'h', redraw_all!()
    macro_rules! show_help {
        () => {
            stdo.queue(cursor::Hide).unwrap();
            erase!();
            redraw(&mut stdo, String::from(HELP_TEXT), None);
            loop {
                match get_event(None) {
                    Some(KE!('h')) => break,
                    Some(Event::Resize(_, _)) => {
                        erase!();
                        redraw(&mut stdo, String::from(HELP_TEXT), None);
                    },
                    _ => (),
                }
            }
            stdo.queue(cursor::Show).unwrap();
            // in case the window resized.
            resize!();
        }
    }

    // start off with control screen. First impressions are important.
    show_help!();

    // main loop
    loop {
        let (mut cur_col, mut cur_row) = cursor::position().unwrap();

        // don't let cursor into toolbar
        if cur_row > rows - 2 {
            stdo.execute(cursor::MoveUp(1)).unwrap();
            // no way to update mutables from tuple?
            let (ncur_col, ncur_row) = cursor::position().unwrap();
            cur_col = ncur_col;
            cur_row = ncur_row;
        }

        match get_event(None) {
            // movement
            Some(KE!('w')) => {stdo.execute(cursor::MoveUp(1)).unwrap();},
            Some(KE!('a')) => {stdo.execute(cursor::MoveLeft(1)).unwrap();},
            Some(KE!('s')) => {stdo.execute(cursor::MoveDown(1)).unwrap();},
            Some(KE!('d')) => {stdo.execute(cursor::MoveRight(1)).unwrap();},

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
                let min_delay = Duration::from_micros(0);
                let max_delay = Duration::from_secs_f64(1./framerates[framerate]);
                let mut poll_time = min_delay;
                let mut delta: Duration;
                // for framerate average. only used if log
                let mut frames = 0.;
                let total_timer = Instant::now();

                loop {
                    let delta_timer = Instant::now();
                    match get_event(Some(poll_time)) {

                        // if 'f', break
                        Some(Event::Key(
                        KeyEvent{
                            code: KeyCode::Char('f'),
                            modifiers: _
                        })) => break,

                        // if resize, resize!
                        Some(Event::Resize(c, r)) => {resize!(c, r);},

                        // else, iter.
                        _ => {
                            // if statement cause log is messy. don't want perpetually growing
                            // vectors in normal play. also theoretically boost performance by not
                            // making so many new timers every frame
                            if log {
                                let step_timer = Instant::now();
                                step!();
                                step_times.push(step_timer.elapsed().as_micros());
                                let draw_timer = Instant::now();
                                redraw_all!();
                                draw_times.push(draw_timer.elapsed().as_micros());
                                frames += 1.;
                            } else {
                                step!();
                                redraw_all!();
                            }
                            delta = delta_timer.elapsed();
                            poll_time = if max_delay > delta {max_delay - delta}
                                        else {min_delay}
                        },
                    } // match end
                } // loop end
                stdo.execute(cursor::Show).unwrap();
                if log {framerate_averages.push(frames/total_timer.elapsed().as_secs() as f64)}
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

            Some(Event::Resize(c, r)) => {resize!(c, r);},

            // quit
            // TODO ctrl-c support. use ctrl-c in play and help loops, too.
            Some(KE!('q')) => match get_event(None) {
                Some(KE!('q')) => break,
                _ => (),
            },

            _ => (),
        } // match end
    } // loop end

    // cleanup
    stdo.execute(terminal::LeaveAlternateScreen).unwrap();
    terminal::disable_raw_mode().unwrap();

    if log {
        if step_times.len() > 1 {
            step_times.sort();
            println!("Step time median:\n{} microseconds\n", step_times[step_times.len()/2]);
        }
        if draw_times.len() > 1 {
            draw_times.sort();
            println!("Draw time median:\n{} microseconds\n", draw_times[draw_times.len()/2]);
        }
        println!("Playback average framerates:\n{:?}", framerate_averages);
    }
}
