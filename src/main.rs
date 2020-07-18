use pancurses::Input;


//// Logic FNs ////

// creates a new grid of x/y size optionally taking extra data from another grid
fn gen_grid(x: usize, y: usize, grid: Option<Vec<Vec<bool>>>) ->  Vec<Vec<bool>> {
    match grid {
        Some(mut data) => {
            for col in &mut data{
                col.resize(x, false)
            }
            data.resize(y, vec![false; x]);
            data
        }
        None => vec![vec![false; x]; y],
    }
}


// Toggles a point on the grid between true and false
fn grid_toggle(grid: &mut Vec<Vec<bool>>, col: usize, row: usize) {
    grid[row][col] = !grid[row][col]
}


// Returns a grid advanced one step in the GOL
fn gol_step(grid: &Vec<Vec<bool>>, live: i32, birth: i32) -> Vec<Vec<bool>> {
    // Blank grid to put results in. Avoids messing with the grid while in-use.
    let mut result = gen_grid(grid[0].len(), grid.len(), None);

    // cast to i32's so subtractions don't panic.
    // Unfortunately means recasting as usize later. Doesn't matter since get() bounds checks,
    // and I strongly doubt someone has a screen size of a few billion tiles.
    let max_x = grid[0].len() as i32;
    let max_y = grid.len() as i32;

    // this part should be thread-able but right now it runs fast enough without
    // so that's for another time.
    for x in 0..max_x {
        for y in 0..max_y {
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
                result[y as usize][x as usize] = true;
            } else if neighbors >= live && neighbors < birth {
                result[y as usize][x as usize] = grid[y as usize][x as usize];
            };

        } }

    result
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


// clears window and redraws text.
fn redraw(window: &pancurses::Window, text: &str) {
    let (cy, cx) = window.get_cur_yx();
    window.erase();
    window.addstr(text);
    window.mv(cy, cx);
    window.refresh();
}


// returns true if char is an acceptable display character
fn valid_chars(c: char) -> bool{
    c.is_alphanumeric() || c.is_whitespace() || c.is_ascii_punctuation()
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


const LOG: bool = true;


fn main() {
    let mut ch_t = 'O';
    let mut ch_f = ' ';
    let mut live: i32 = 2;
    let mut birth: i32 = 3;
    let framerates = [0.5, 1., 2., 5., 10., 15., 20., 30., 45., 60., 90., 120., 999.];
    let mut framerate = 5;
    let mut window = pancurses::initscr();
    pancurses::noecho();
    let (mut cols, mut rows) = window.get_max_yx();

    let mut matrix = gen_grid(rows as usize, cols as usize - 1, None);

    let mut draw_times = Vec::<u128>::new();
    let mut step_times = Vec::<u128>::new();

    macro_rules! redraw_all {
        () => {
            redraw(&window, &(grid_to_str(&matrix, ch_t, ch_f)+&gen_toolbar(ch_t, ch_f, live, birth, framerates[framerate])));
        }
    }

    macro_rules! show_help {
        () => {
            pancurses::curs_set(0);
            redraw(&window, HELP_TEXT);
            while window.getch() != Some(Input::Character('h')) {}
            pancurses::curs_set(1);
            redraw_all!();
        }
    }

    window.mv(cols/2, rows/2);
    show_help!();

    loop {
        let (cur_row, cur_col) = window.get_cur_yx();

        match window.getch() {
            // movement
            Some(Input::Character('w')) => {window.mv(cur_row-1, cur_col);},
            Some(Input::Character('a')) => {window.mv(cur_row, cur_col-1);},
            // make sure it doesn't clip toolbar. Laziest way possible.
            // TODO maybe the main win should be 1 less than window, and toolbar should be its own win?
            Some(Input::Character('s')) => {window.mv(
                std::cmp::min(cur_row+1, matrix.len() as i32 - 1), cur_col);},
            Some(Input::Character('d')) => {window.mv(cur_row, cur_col+1);},

            // toggle point
            Some(Input::Character(' ')) => {
                grid_toggle(&mut matrix, cur_col as usize, cur_row as usize);
                redraw_all!();
            },

            // change rules
            Some(Input::Character('-')) => {
                live = (live-1).max(0);
                redraw_all!();
            },
            Some(Input::Character('=')) => {
                live = (live+1).min(9);
                redraw_all!();
            },
            Some(Input::Character('[')) => {
                birth = (birth-1).max(0);
                redraw_all!();
            },
            Some(Input::Character(']')) => {
                birth = (birth+1).min(9);
                redraw_all!();
            },

            // frame-advance
            Some(Input::Character('e')) =>  {
                matrix = gol_step(&matrix, live, birth);
                redraw_all!();
            }

            // change framerate
            Some(Input::Character(',')) => {
                framerate = (framerate as i32-1).max(0) as usize;
                redraw_all!();
            }
            Some(Input::Character('.')) => {
                framerate = (framerate+1).min(framerates.len()-1);
                redraw_all!();
            }

            // play
            Some(Input::Character('f')) =>  {
                pancurses::curs_set(0);
                window.timeout(0);
                let max_delay = ((1./framerates[framerate]) * 1000.) as i32;
                while window.getch() != Some(Input::Character('f')){
                    let now = std::time::Instant::now();
                    if LOG {
                        let step_timer = std::time::Instant::now();
                        matrix = gol_step(&matrix, live, birth);
                        step_times.push(step_timer.elapsed().as_micros());
                        let draw_timer = std::time::Instant::now();
                        redraw_all!();
                        draw_times.push(draw_timer.elapsed().as_micros());
                    } else {
                        matrix = gol_step(&matrix, live, birth);
                        redraw_all!();
                    }
                    window.timeout((max_delay - now.elapsed().as_millis() as i32).max(0));
                }
                window.timeout(-1);
                pancurses::curs_set(1);
            }

            // clear
            Some(Input::Character('x')) => {
                match window.getch() {
                    Some(Input::Character('x')) => {
                        matrix = gen_grid(rows as usize, cols as usize - 1, None);
                        redraw_all!();
                    },
                    _ => (),
                }
            }

            // change chars
            Some(Input::Character('c')) => {
                window.mv(cols-1, 4);
                match window.getch() {
                    Some(Input::Character(c)) => {
                        if valid_chars(c) && c != ch_f {ch_t = c;}
                    }
                    _ => ()
                }
                redraw_all!();
                window.mv(cols-1, 11);
                match window.getch() {
                    Some(Input::Character(c)) => {
                        if valid_chars(c) && c != ch_t {ch_f = c;}
                    }
                    _ => ()
                }
                window.mv(cur_row, cur_col);
                redraw_all!();
            }

            // show/hide help.
            Some(Input::Character('h')) => {
                show_help!();
            }

            // TODO resize. Seems like it's missing a lot of resize fns in the docs.
            // Blocking issue for 1.0
            Some(Input::KeyResize) => {
                window.clear();
                window.refresh();
                // window.delwin();
                pancurses::resize_term(0, 0);
                window = pancurses::newwin(0, 0, 0, 0);

                cols = window.get_max_y();
                rows = window.get_max_x();
                matrix = gen_grid(rows as usize, cols as usize - 1, Some(matrix));
                redraw_all!();
            }

            // quit
            Some(Input::Character('q')) => {
                match window.getch() {
                    Some(Input::Character('q')) => break,
                    _ => (),
                }
            }

            _ => (),
        }
    }

    pancurses::endwin();
    if LOG {
        step_times.sort();
        draw_times.sort();
        println!("Step Median:\n{}\n\nDraw Median:\n{}", step_times[step_times.len()/2], draw_times[draw_times.len()/2]);
    }
}
