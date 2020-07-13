# cursed_life 0.9.0
basically game of life in ncurses. doesn't use cutting-edge algorithms or anything. more of a small Rust exercise than anything.

## Info
### Controls
 - wasd  : move
 - space : toggle gridpoint
 - e     : frame advance
 - f     : playback. I don't like this binding, might change it.
 - xx    : clear
 - qq    : quit

Game of Life rules. Values above 8 or below 1 don't make any sense, but right now there's no bounds checking so...
 - minus/equals '-='  : adjust 'lives' rule
 - brackets '[]'      : adjust 'birth' rule

### Playing
Just run the file, use space to manually add lifeforms, and press e or f to advance. Don't resize the window, that's not implemented yet.

### Building
Uses `pancurses` crate which claims to be platform-agnostic. Can guarantee it works in Linux.
