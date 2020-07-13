use pancurses::Input;


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
            } else if neighbors == live {
                result[y as usize][x as usize] = grid[y as usize][x as usize];
            };

        } }

    result
}


// creates the string for the toolbar.
fn gen_toolbar<T: std::fmt::Display>(live: T, birth: T) -> String{
    format!("live:{}  birth:{}", live, birth)
}


// clears window and redraws text.
fn redraw(window: &pancurses::Window, text: &str) {
    let (cy, cx) = window.get_cur_yx();
    window.erase();
    window.addstr(text);
    window.mv(cy, cx);
    window.refresh();
}


fn main() {
    let ch_t = 'O';
    let ch_f = ' ';
    let mut live = 2;
    let mut birth = 3;
    let window = pancurses::initscr();
    pancurses::noecho();
    let (cols, rows) = window.get_max_yx();

    let mut matrix = gen_grid(rows as usize, cols as usize - 1, None);

    macro_rules! redraw_all {
        () => {
            redraw(&window, &(grid_to_str(&matrix, ch_t, ch_f)+&gen_toolbar(live, birth)));
        }
    }

    window.mv(cols/2, rows/2);
    redraw_all!();

    loop {
        let (cur_row, cur_col) = window.get_cur_yx();

        match window.getch() {
            // movement
            Some(Input::Character('w')) => {window.mv(cur_row-1, cur_col);},
            Some(Input::Character('a')) => {window.mv(cur_row, cur_col-1);},
            // make sure it doesn't clip toolbar. Laziest way possible.
            // maybe the main win should be 1 less than window, and toolbar should be its own win?
            Some(Input::Character('s')) => {window.mv(
                std::cmp::min(cur_row+1, matrix.len() as i32 - 1), cur_col);},
            Some(Input::Character('d')) => {window.mv(cur_row, cur_col+1);},

            // change rules
            Some(Input::Character('-')) => {live -= 1; redraw_all!()},
            Some(Input::Character('=')) => {live += 1; redraw_all!()},
            Some(Input::Character('[')) => {birth -= 1; redraw_all!()},
            Some(Input::Character(']')) => {birth += 1; redraw_all!()},

            // toggle point
            Some(Input::Character(' ')) => {
                grid_toggle(&mut matrix, cur_col as usize, cur_row as usize);
                redraw_all!();
            },

            // frame-advance
            Some(Input::Character('e')) =>  {
                matrix = gol_step(&matrix, live, birth);
                redraw_all!();
            }

            // play
            Some(Input::Character('f')) =>  {
                pancurses::curs_set(0);
                window.timeout(0);
                while window.getch() != Some(Input::Character('f')){
                    matrix = gol_step(&matrix, live, birth);
                    redraw_all!();
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
}
