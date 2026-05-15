#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ncurses.h>
#include <unistd.h>
#include <math.h>

#define PADDLE_HEIGHT 7
#define PADDLE_WIDTH 1
#define BALL_CHAR 'O'
#define INITIAL_SPEED 2
#define MAX_SPEED 8
#define WIN_SCORE 10
#define AI_SPEED 2

typedef enum {
    STATE_WAITING,
    STATE_PLAYING,
    STATE_GAME_OVER
} GameState;

typedef struct {
    int x;
    int y;
    int speed;
    int dir_x;
    int dir_y;
} Ball;

typedef struct {
    int x;
    int y;
    int height;
} Paddle;

typedef struct {
    Ball ball;
    Paddle player;
    Paddle ai;
    int score_player;
    int score_ai;
    GameState state;
    int winner;
} Game;

static void reset_ball(Game *game);

static void init_game(Game *game) {
    memset(game, 0, sizeof(Game));
    game->state = STATE_WAITING;
    game->score_player = 0;
    game->score_ai = 0;
    game->winner = 0;

    game->player.x = 3;
    game->player.y = LINES / 2 - PADDLE_HEIGHT / 2;
    game->player.height = PADDLE_HEIGHT;

    game->ai.x = COLS - 4;
    game->ai.y = LINES / 2 - PADDLE_HEIGHT / 2;
    game->ai.height = PADDLE_HEIGHT;

    reset_ball(game);
}

static void reset_ball(Game *game) {
    game->ball.x = COLS / 2;
    game->ball.y = LINES / 2;
    game->ball.speed = INITIAL_SPEED;
    game->ball.dir_x = (rand() % 2 == 0) ? 1 : -1;
    game->ball.dir_y = (rand() % 2 == 2) ? 1 : -1;
}

static void move_ai(Game *game) {
    int center = game->ai.y + game->ai.height / 2;
    int target = game->ball.y;
    int diff = target - center;

    if (diff > AI_SPEED)
        game->ai.y += AI_SPEED;
    else if (diff < -AI_SPEED)
        game->ai.y -= AI_SPEED;
    else
        game->ai.y += (diff > 0) ? 1 : -1;

    if (game->ai.y < 1) game->ai.y = 1;
    if (game->ai.y + game->ai.height > LINES - 1)
        game->ai.y = LINES - 1 - game->ai.height;
}

static void move_player(Game *game) {
    if (game->player.y < 1)
        game->player.y = 1;
    if (game->player.y + game->player.height > LINES - 1)
        game->player.y = LINES - 1 - game->player.height;
}

static int check_paddle_hit(Game *game, Paddle *paddle) {
    return (game->ball.x == paddle->x &&
            game->ball.y >= paddle->y &&
            game->ball.y < paddle->y + paddle->height);
}

static void move_ball(Game *game) {
    game->ball.x += game->ball.dir_x * game->ball.speed;
    game->ball.y += game->ball.dir_y * game->ball.speed;

    if (game->ball.y <= 1 || game->ball.y >= LINES - 2) {
        game->ball.dir_y *= -1;
        game->ball.y = (game->ball.y <= 1) ? 1 : LINES - 2;
    }

    if (check_paddle_hit(game, &game->player)) {
        game->ball.dir_x = 1;
        float pos = (float)(game->ball.y - game->player.y) / game->player.height;
        game->ball.dir_y = (int)((pos - 0.5) * 6);
        if (game->ball.speed < MAX_SPEED)
            game->ball.speed++;
    }

    if (check_paddle_hit(game, &game->ai)) {
        game->ball.dir_x = -1;
        float pos = (float)(game->ball.y - game->ai.y) / game->ai.height;
        game->ball.dir_y = (int)((pos - 0.5) * 6);
        if (game->ball.speed < MAX_SPEED)
            game->ball.speed++;
    }

    if (game->ball.x <= 1) {
        game->score_ai++;
        if (game->score_ai >= WIN_SCORE) {
            game->state = STATE_GAME_OVER;
            game->winner = 2;
        } else {
            reset_ball(game);
            move_ai(game);
        }
    }

    if (game->ball.x >= COLS - 2) {
        game->score_player++;
        if (game->score_player >= WIN_SCORE) {
            game->state = STATE_GAME_OVER;
            game->winner = 1;
        } else {
            reset_ball(game);
            move_ai(game);
        }
    }
}

static void draw_field(Game *game) {
    for (int y = 0; y < LINES; y++) {
        for (int x = 0; x < COLS; x++) {
            if (x == 0 || x == COLS - 1) {
                mvaddch(y, x, '|');
            } else if (x == COLS / 2) {
                mvaddch(y, x, (y % 2 == 0) ? ':' : '.');
            }
        }
    }

    for (int i = 0; i < game->player.height; i++) {
        mvaddch(game->player.y + i, game->player.x, '#');
    }

    for (int i = 0; i < game->ai.height; i++) {
        mvaddch(game->ai.y + i, game->ai.x, '#');
    }

    mvaddch(game->ball.y, game->ball.x, BALL_CHAR);
}

static void draw_score(Game *game) {
    char buf[32];
    snprintf(buf, sizeof(buf), " %d | %d ", game->score_player, game->score_ai);
    mvaddstr(0, COLS / 2 - 6, buf);
}

static void draw_message(Game *game) {
    char buf[128];
    if (game->state == STATE_WAITING) {
        snprintf(buf, sizeof(buf), " Pong - Press Space to start (W/S to move, Q to quit) ");
    } else if (game->state == STATE_GAME_OVER) {
        snprintf(buf, sizeof(buf), " Player %d wins! Press Space to play again, Q to quit ", game->winner);
    }
    if (strlen(buf) > 0) {
        int y = LINES / 2;
        int x = (COLS - (int)strlen(buf)) / 2;
        if (x >= 0) {
            attron(A_BOLD | A_REVERSE);
            mvaddstr(y, x, buf);
            attroff(A_BOLD | A_REVERSE);
        }
    }
}

static void handle_input(Game *game) {
    int ch = getch();
    if (ch == 'q' || ch == 'Q') {
        endwin();
        exit(0);
    }

    if (ch == 'w' || ch == 'W')
        game->player.y--;
    if (ch == 's' || ch == 'S')
        game->player.y++;

    if (ch == ' ') {
        if (game->state == STATE_WAITING)
            game->state = STATE_PLAYING;
        else if (game->state == STATE_GAME_OVER)
            init_game(game);
    }
}

static void draw_title(void) {
    const char *title = "  .oO Pong Oo. ";
    mvaddstr(0, COLS / 2 - (int)strlen(title) / 2, title);
}

int main(void) {
    initscr();
    cbreak();
    noecho();
    curs_set(0);
    keypad(stdscr, TRUE);
    nodelay(stdscr, TRUE);
    start_color();
    init_pair(1, COLOR_GREEN, COLOR_BLACK);
    init_pair(2, COLOR_YELLOW, COLOR_BLACK);
    init_pair(3, COLOR_RED, COLOR_BLACK);

    Game game;
    init_game(&game);

    while (1) {
        clear();
        draw_title();
        draw_score(&game);
        draw_field(&game);

        if (game.state == STATE_WAITING || game.state == STATE_GAME_OVER) {
            draw_message(&game);
        }

        refresh();

        handle_input(&game);

        if (game.state == STATE_PLAYING) {
            move_player(&game);
            move_ai(&game);
            move_ball(&game);
        }

        usleep(50000);
    }

    endwin();
    return 0;
}
