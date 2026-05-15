#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ncurses.h>
#include <unistd.h>
#include <time.h>

#define GRAVITY 0.3
#define MARIO_SPEED 3
#define MARIO_JUMP -7
#define MARIO_FRICTION 0.85
#define TILE 3
#define WORLD_W 120
#define WORLD_H 20
#define SCREEN_W 60
#define SCREEN_H 22
#define MAX_LIVES 3
#define MAX_LEVELS 10

typedef enum {
    STATE_TITLE,
    STATE_PLAYING,
    STATE_LEVEL_DONE,
    STATE_GAME_OVER,
    STATE_WIN
} GameState;

typedef enum {
    TILE_EMPTY = 0,
    TILE_GROUND,
    TILE_BRICK,
    TILE_PIPE,
    TILE_FLAG,
    TILE_PLATFORM,
    TILE_MOVE_PLATFORM
} TileType;

typedef struct {
    int x, y;
    int w, h;
    TileType type;
    int dir;
} Tile;

typedef struct {
    int x, y;
    int vx, vy;
    int facing;
    int grounded;
    int jump_held;
    int invincible;
    int alive;
} Mario;

typedef struct {
    int x, y;
    int vx, vy;
    int alive;
    int type;
    int width;
} Enemy;

typedef struct {
    int x, y;
    int alive;
} Coin;

typedef struct {
    int x, y;
    int reached;
} Flag;

typedef struct {
    int x;
    int y;
    int dir;
    int speed;
} MovingPlatform;

typedef struct {
    Tile tiles[1000];
    int tile_count;
    Enemy enemies[20];
    int enemy_count;
    Coin coins[200];
    int coin_count;
    Flag flag;
    MovingPlatform platforms[10];
    int platform_count;
} Level;

typedef struct {
    GameState state;
    Mario mario;
    int lives;
    int score;
    int coins;
    int level;
    int camera_x;
    Level levels[MAX_LEVELS];
    int level_done_timer;
} Game;

static int is_walkable(TileType t) {
    return t == TILE_GROUND || t == TILE_BRICK || t == TILE_PIPE || t == TILE_PLATFORM || t == TILE_MOVE_PLATFORM;
}

static Tile get_tile(Level *lv, int x, int y) {
    for (int i = 0; i < lv->tile_count; i++) {
        Tile *t = &lv->tiles[i];
        if (x >= t->x && x < t->x + t->w && y >= t->y && y < t->y + t->h)
            return *t;
    }
    Tile empty = {0, 0, 0, 0, TILE_EMPTY};
    return empty;
}

static void add_tile(Level *lv, int x, int y, int w, int h, TileType t) {
    if (lv->tile_count < 1000) {
        lv->tiles[lv->tile_count++] = (Tile){x, y, w, h, t, 1};
    }
}

static void add_coin(Level *lv, int x, int y) {
    if (lv->coin_count < 200) {
        lv->coins[lv->coin_count++] = (Coin){x, y, 1};
    }
}

static void add_enemy(Level *lv, int x, int y, int type) {
    if (lv->enemy_count < 20) {
        lv->enemies[lv->enemy_count++] = (Enemy){x, y, -1, 0, 1, type, 2};
    }
}

static void add_platform(Level *lv, int x, int y, int w, int dir, int speed) {
    if (lv->platform_count < 10) {
        lv->platforms[lv->platform_count++] = (MovingPlatform){x, y, dir, speed};
    }
}

static void build_level(Level *lv, int level) {
    lv->tile_count = 0;
    lv->enemy_count = 0;
    lv->coin_count = 0;
    lv->platform_count = 0;
    lv->flag.x = WORLD_W - 5;
    lv->flag.y = 15;
    lv->flag.reached = 0;

    int difficulty = level;

    for (int x = 0; x < WORLD_W; x++) {
        if (x >= 50 && x <= 52 && level >= 2) continue;
        if (x >= 80 && x <= 83 && level >= 3) continue;
        if (x >= 105 && x <= 107 && level >= 4) continue;

        add_tile(lv, x, 18, 1, 2, TILE_GROUND);
        add_tile(lv, x, 16, 1, 2, TILE_GROUND);
    }

    if (level >= 1) {
        add_tile(lv, 10, 14, 4, 1, TILE_BRICK);
        add_tile(lv, 15, 11, 3, 1, TILE_BRICK);
        add_tile(lv, 25, 14, 2, 1, TILE_BRICK);
        add_tile(lv, 35, 12, 4, 1, TILE_BRICK);
        add_tile(lv, 45, 13, 3, 1, TILE_BRICK);
        add_tile(lv, 60, 14, 5, 1, TILE_BRICK);
        add_tile(lv, 75, 11, 3, 1, TILE_BRICK);
        add_tile(lv, 90, 13, 4, 1, TILE_BRICK);
        add_tile(lv, 110, 12, 3, 1, TILE_BRICK);
    }

    if (level >= 2) {
        add_tile(lv, 12, 10, 1, 1, TILE_PIPE);
        add_tile(lv, 12, 11, 1, 1, TILE_PIPE);
        add_tile(lv, 12, 12, 1, 1, TILE_PIPE);
        add_tile(lv, 12, 13, 1, 1, TILE_PIPE);

        add_tile(lv, 30, 9, 1, 1, TILE_PIPE);
        add_tile(lv, 30, 10, 1, 1, TILE_PIPE);
        add_tile(lv, 30, 11, 1, 1, TILE_PIPE);
        add_tile(lv, 30, 12, 1, 1, TILE_PIPE);
        add_tile(lv, 30, 13, 1, 1, TILE_PIPE);

        add_tile(lv, 55, 10, 1, 1, TILE_PIPE);
        add_tile(lv, 55, 11, 1, 1, TILE_PIPE);
        add_tile(lv, 55, 12, 1, 1, TILE_PIPE);
        add_tile(lv, 55, 13, 1, 1, TILE_PIPE);

        add_tile(lv, 70, 9, 1, 1, TILE_PIPE);
        add_tile(lv, 70, 10, 1, 1, TILE_PIPE);
        add_tile(lv, 70, 11, 1, 1, TILE_PIPE);
        add_tile(lv, 70, 12, 1, 1, TILE_PIPE);
        add_tile(lv, 70, 13, 1, 1, TILE_PIPE);
    }

    if (level >= 3) {
        for (int i = 0; i < 4; i++) {
            add_tile(lv, 40 + i * 2, 14 - i, 1, 1, TILE_BRICK);
        }
        add_tile(lv, 50, 13, 3, 1, TILE_BRICK);
        add_tile(lv, 65, 12, 2, 1, TILE_BRICK);
        add_tile(lv, 85, 10, 3, 1, TILE_BRICK);
        add_tile(lv, 95, 14, 4, 1, TILE_BRICK);
    }

    if (level >= 4) {
        add_platform(lv, 20, 13, 3, 1, 1);
        add_platform(lv, 45, 12, 3, 1, 2);
        add_platform(lv, 70, 11, 3, -1, 1);
        add_platform(lv, 95, 13, 3, 1, 2);
    }

    if (level >= 5) {
        add_tile(lv, 18, 15, 2, 1, TILE_BRICK);
        add_tile(lv, 28, 13, 3, 1, TILE_BRICK);
        add_tile(lv, 38, 11, 2, 1, TILE_BRICK);
        add_tile(lv, 48, 14, 4, 1, TILE_BRICK);
        add_tile(lv, 58, 12, 3, 1, TILE_BRICK);
        add_tile(lv, 68, 10, 2, 1, TILE_BRICK);
        add_tile(lv, 78, 13, 4, 1, TILE_BRICK);
        add_tile(lv, 88, 11, 3, 1, TILE_BRICK);
        add_tile(lv, 100, 12, 2, 1, TILE_BRICK);
    }

    if (level >= 6) {
        add_tile(lv, 15, 16, 2, 1, TILE_BRICK);
        add_tile(lv, 25, 15, 2, 1, TILE_BRICK);
        add_tile(lv, 35, 14, 2, 1, TILE_BRICK);
        add_tile(lv, 45, 13, 2, 1, TILE_BRICK);
        add_tile(lv, 55, 12, 2, 1, TILE_BRICK);
        add_tile(lv, 65, 11, 2, 1, TILE_BRICK);
        add_tile(lv, 75, 10, 2, 1, TILE_BRICK);
        add_tile(lv, 85, 14, 3, 1, TILE_BRICK);
        add_tile(lv, 95, 12, 2, 1, TILE_BRICK);
        add_tile(lv, 105, 11, 2, 1, TILE_BRICK);
    }

    if (level >= 7) {
        add_tile(lv, 20, 14, 3, 1, TILE_BRICK);
        add_tile(lv, 30, 12, 2, 1, TILE_BRICK);
        add_tile(lv, 40, 10, 3, 1, TILE_BRICK);
        add_tile(lv, 50, 13, 2, 1, TILE_BRICK);
        add_tile(lv, 60, 11, 4, 1, TILE_BRICK);
        add_tile(lv, 70, 9, 2, 1, TILE_BRICK);
        add_tile(lv, 80, 12, 3, 1, TILE_BRICK);
        add_tile(lv, 90, 10, 2, 1, TILE_BRICK);
        add_tile(lv, 100, 13, 3, 1, TILE_BRICK);
    }

    if (level >= 8) {
        add_platform(lv, 15, 14, 2, 1, 2);
        add_platform(lv, 30, 12, 2, -1, 2);
        add_platform(lv, 45, 10, 2, 1, 2);
        add_platform(lv, 60, 13, 2, -1, 2);
        add_platform(lv, 75, 11, 2, 1, 2);
        add_platform(lv, 90, 14, 2, -1, 2);
        add_platform(lv, 105, 12, 2, 1, 2);
    }

    if (level >= 9) {
        add_tile(lv, 22, 15, 2, 1, TILE_BRICK);
        add_tile(lv, 32, 13, 2, 1, TILE_BRICK);
        add_tile(lv, 42, 11, 2, 1, TILE_BRICK);
        add_tile(lv, 52, 14, 2, 1, TILE_BRICK);
        add_tile(lv, 62, 12, 2, 1, TILE_BRICK);
        add_tile(lv, 72, 10, 2, 1, TILE_BRICK);
        add_tile(lv, 82, 13, 2, 1, TILE_BRICK);
        add_tile(lv, 92, 11, 2, 1, TILE_BRICK);
        add_tile(lv, 102, 14, 2, 1, TILE_BRICK);
    }

    if (level >= 10) {
        add_tile(lv, 20, 14, 3, 1, TILE_BRICK);
        add_tile(lv, 30, 12, 3, 1, TILE_BRICK);
        add_tile(lv, 40, 10, 3, 1, TILE_BRICK);
        add_tile(lv, 50, 13, 3, 1, TILE_BRICK);
        add_tile(lv, 60, 11, 3, 1, TILE_BRICK);
        add_tile(lv, 70, 9, 3, 1, TILE_BRICK);
        add_tile(lv, 80, 12, 3, 1, TILE_BRICK);
        add_tile(lv, 90, 10, 3, 1, TILE_BRICK);
        add_tile(lv, 100, 13, 3, 1, TILE_BRICK);
        add_platform(lv, 25, 13, 2, 1, 2);
        add_platform(lv, 45, 11, 2, -1, 2);
        add_platform(lv, 65, 10, 2, 1, 2);
        add_platform(lv, 85, 12, 2, -1, 2);
        add_platform(lv, 105, 14, 2, 1, 2);
    }

    int num_enemies = 3 + level;
    for (int i = 0; i < num_enemies; i++) {
        int ex = 10 + (i * (WORLD_W - 20)) / num_enemies;
        add_enemy(lv, ex, 15, 1);
    }

    int num_coins = 5 + level * 2;
    for (int i = 0; i < num_coins; i++) {
        add_coin(lv, 8 + i * 11, 12 + (i % 3) * 2);
    }
}

static void init_game(Game *game) {
    memset(game, 0, sizeof(Game));
    game->state = STATE_TITLE;
    game->lives = MAX_LIVES;
    game->score = 0;
    game->coins = 0;
    game->level = 0;
    game->level_done_timer = 0;

    for (int i = 0; i < MAX_LEVELS; i++) {
        build_level(&game->levels[i], i);
    }

    game->mario.x = 3;
    game->mario.y = 15;
    game->mario.vx = 0;
    game->mario.vy = 0;
    game->mario.facing = 1;
    game->mario.grounded = 0;
    game->mario.jump_held = 0;
    game->mario.invincible = 0;
    game->mario.alive = 1;
}

static void reset_mario(Game *game) {
    game->mario.x = 3;
    game->mario.y = 15;
    game->mario.vx = 0;
    game->mario.vy = 0;
    game->mario.facing = 1;
    game->mario.grounded = 0;
    game->mario.jump_held = 0;
    game->mario.invincible = 180;
    game->mario.alive = 1;
    game->camera_x = 0;
}

static void move_platforms(Level *lv) {
    for (int i = 0; i < lv->platform_count; i++) {
        MovingPlatform *mp = &lv->platforms[i];
        mp->x += mp->dir * mp->speed;
        if (mp->x < 0 || mp->x + 3 > WORLD_W) mp->dir *= -1;
    }
}

static int is_on_platform(Game *game, int mx, int my) {
    Level *lv = &game->levels[game->level];

    for (int i = 0; i < lv->platform_count; i++) {
        MovingPlatform *mp = &lv->platforms[i];
        if (mx >= mp->x && mx < mp->x + 3 && my == mp->y) {
            return 1;
        }
    }
    return 0;
}

static void update_mario(Game *game) {
    Mario *m = &game->mario;
    Level *lv = &game->levels[game->level];

    if (m->invincible > 0) m->invincible--;

    int key = getch();
    if (key == KEY_LEFT) {
        m->vx -= 1;
        m->facing = -1;
    }
    if (key == KEY_RIGHT) {
        m->vx += 1;
        m->facing = 1;
    }
    if (key == KEY_UP) {
        if (m->grounded) {
            m->vy = MARIO_JUMP;
            m->grounded = 0;
        }
    }
    if (key == 'q' || key == 'Q') {
        endwin();
        exit(0);
    }
    if (key == ' ') {
        if (game->state == STATE_TITLE) game->state = STATE_PLAYING;
        else if (game->state == STATE_LEVEL_DONE) {
            game->level++;
            if (game->level >= MAX_LEVELS) {
                game->state = STATE_WIN;
            } else {
                game->state = STATE_PLAYING;
                reset_mario(game);
            }
        } else if (game->state == STATE_GAME_OVER) {
            init_game(game);
        } else if (game->state == STATE_WIN) {
            init_game(game);
        }
    }

    m->vx *= MARIO_FRICTION;
    if (m->vx > MARIO_SPEED) m->vx = MARIO_SPEED;
    if (m->vx < -MARIO_SPEED) m->vx = -MARIO_SPEED;

    m->vy += GRAVITY;
    if (m->vy > 10) m->vy = 10;

    m->x += m->vx;
    m->y += m->vy;

    m->grounded = 0;

    Tile t = get_tile(lv, m->x, m->y);
    if (is_walkable(t.type)) {
        m->x -= m->vx;
        m->vx = 0;
    }

    t = get_tile(lv, m->x, m->y + 1);
    if (is_walkable(t.type)) {
        m->y -= m->vy;
        m->vy = 0;
        m->grounded = 1;
    }

    t = get_tile(lv, m->x, m->y - 1);
    if (is_walkable(t.type)) {
        m->y += 1;
        m->vy = 0;
    }

    if (m->y > WORLD_H + 2) {
        game->lives--;
        if (game->lives <= 0) {
            game->state = STATE_GAME_OVER;
        } else {
            reset_mario(game);
        }
    }

    if (m->x < 0) m->x = 0;

    if (m->x >= lv->flag.x && m->x <= lv->flag.x + 1 && m->y >= lv->flag.y - 2) {
        lv->flag.reached = 1;
        game->state = STATE_LEVEL_DONE;
        game->level_done_timer = 120;
        game->score += 500;
    }

    if (m->x > game->camera_x + SCREEN_W - 5) {
        game->camera_x = m->x - SCREEN_W / 2;
    }
    if (game->camera_x < 0) game->camera_x = 0;
}

static void update_enemies(Game *game) {
    Level *lv = &game->levels[game->level];
    Mario *m = &game->mario;

    for (int i = 0; i < lv->enemy_count; i++) {
        Enemy *e = &lv->enemies[i];
        if (!e->alive) continue;

        e->x += e->vx;
        e->y += e->vy;
        e->vy += GRAVITY;

        Tile t = get_tile(lv, e->x, e->y + 1);
        if (is_walkable(t.type)) {
            e->y -= e->vy;
            e->vy = 0;
        }

        t = get_tile(lv, e->x + e->vx, e->y);
        if (is_walkable(t.type)) {
            e->vx *= -1;
        }

        if (e->x < 0 || e->x > WORLD_W) e->vx *= -1;

        int dx = m->x - e->x;
        int dy = m->y - e->y;
        if (dx * dx + dy * dy < 9) {
            if (m->vy > 0 && m->y < e->y) {
                e->alive = 0;
                m->vy = -5;
                game->score += 200;
            } else if (m->invincible <= 0) {
                game->lives--;
                if (game->lives <= 0) {
                    game->state = STATE_GAME_OVER;
                } else {
                    reset_mario(game);
                }
            }
        }
    }
}

static void update_coins(Game *game) {
    Level *lv = &game->levels[game->level];
    Mario *m = &game->mario;

    for (int i = 0; i < lv->coin_count; i++) {
        Coin *c = &lv->coins[i];
        if (!c->alive) continue;
        int dx = m->x - c->x;
        int dy = m->y - c->y;
        if (dx * dx + dy * dy < 9) {
            c->alive = 0;
            game->coins++;
            game->score += 100;
        }
    }
}

static void draw_tile(Tile t, int cam_x) {
    int sx = t.x - cam_x;
    int sy = t.y;
    if (sx < -5 || sx > SCREEN_W + 5) return;

    int color = COLOR_WHITE;
    switch (t.type) {
        case TILE_GROUND: color = COLOR_GREEN; break;
        case TILE_BRICK: color = COLOR_RED; break;
        case TILE_PIPE: color = COLOR_CYAN; break;
        case TILE_FLAG: color = COLOR_YELLOW; break;
        case TILE_PLATFORM: color = COLOR_MAGENTA; break;
        case TILE_MOVE_PLATFORM: color = COLOR_YELLOW; break;
    }

    attron(COLOR_PAIR(color));
    for (int y = sy; y < sy + t.h; y++) {
        for (int x = sx; x < sx + t.w; x++) {
            if (y >= 0 && y < SCREEN_H && x >= 0 && x < SCREEN_W) {
                mvaddch(y, x, '#');
            }
        }
    }
    attroff(COLOR_PAIR(color));
}

static void draw_mario(Mario *m, int cam_x) {
    int sx = m->x - cam_x;
    int sy = m->y;
    if (sx < -2 || sx > SCREEN_W + 2) return;
    if (m->invincible > 0 && (m->invincible / 4) % 2 == 0) return;

    attron(A_BOLD | COLOR_PAIR(COLOR_RED));
    mvaddch(sy, sx, '@');
    attroff(A_BOLD | COLOR_PAIR(COLOR_RED));
}

static void draw_enemy(Enemy *e, int cam_x) {
    int sx = e->x - cam_x;
    int sy = e->y;
    if (sx < -2 || sx > SCREEN_W + 2) return;

    attron(A_BOLD | COLOR_PAIR(COLOR_BLUE));
    mvaddch(sy, sx, 'E');
    attroff(A_BOLD | COLOR_PAIR(COLOR_BLUE));
}

static void draw_coin(Coin *c, int cam_x) {
    int sx = c->x - cam_x;
    int sy = c->y;
    if (sx < -2 || sx > SCREEN_W + 2) return;

    attron(A_BOLD | COLOR_PAIR(COLOR_YELLOW));
    mvaddch(sy, sx, '$');
    attroff(A_BOLD | COLOR_PAIR(COLOR_YELLOW));
}

static void draw_flag(Flag *f, int cam_x) {
    int sx = f->x - cam_x;
    for (int y = 0; y < f->y; y++) {
        if (sx >= 0 && sx < SCREEN_W) mvaddch(y, sx, '|');
    }
    attron(A_BOLD | COLOR_PAIR(COLOR_WHITE));
    mvaddch(f->y, sx, 'F');
    mvaddch(f->y - 1, sx, '^');
    attroff(A_BOLD | COLOR_PAIR(COLOR_WHITE));
}

static void draw_game(Game *game) {
    clear();
    Level *lv = &game->levels[game->level];

    for (int i = 0; i < lv->tile_count; i++) {
        draw_tile(lv->tiles[i], game->camera_x);
    }

    for (int i = 0; i < lv->platform_count; i++) {
        MovingPlatform *mp = &lv->platforms[i];
        attron(A_BOLD | COLOR_PAIR(COLOR_YELLOW));
        for (int x = mp->x - game->camera_x; x < mp->x - game->camera_x + 3; x++) {
            if (x >= 0 && x < SCREEN_W) mvaddch(mp->y, x, '-');
        }
        attroff(A_BOLD | COLOR_PAIR(COLOR_YELLOW));
    }

    for (int i = 0; i < lv->enemy_count; i++) {
        if (lv->enemies[i].alive) draw_enemy(&lv->enemies[i], game->camera_x);
    }

    for (int i = 0; i < lv->coin_count; i++) {
        if (lv->coins[i].alive) draw_coin(&lv->coins[i], game->camera_x);
    }

    draw_flag(&lv->flag, game->camera_x);
    draw_mario(&game->mario, game->camera_x);

    char buf[256];
    snprintf(buf, sizeof(buf),
        " Level: %d | Lives: %d | Score: %d | Coins: %d ",
        game->level + 1, game->lives, game->score, game->coins);
    mvaddstr(0, 0, buf);

    refresh();
}

static void draw_title(void) {
    clear();
    attron(A_BOLD | COLOR_PAIR(COLOR_GREEN));
    mvaddstr(8, 18, "  SUPER MARIO  ");
    mvaddstr(10, 16, "  10 LEVELS OF FUN!  ");
    mvaddstr(12, 16, "  Arrow Keys: Move & Jump  ");
    mvaddstr(13, 16, "  Space: Start/Restart  ");
    mvaddstr(14, 18, "  Press Space to Play  ");
    attroff(A_BOLD | COLOR_PAIR(COLOR_GREEN));
    refresh();
}

static void draw_level_done(Game *game) {
    clear();
    attron(A_BOLD | COLOR_PAIR(COLOR_GREEN));
    mvaddstr(10, 15, " LEVEL COMPLETE! ");
    mvaddstr(12, 15, " Press Space for next level ");
    attroff(A_BOLD | COLOR_PAIR(COLOR_GREEN));
    refresh();
    sleep(1);
}

static void draw_game_over(void) {
    clear();
    attron(A_BOLD | COLOR_PAIR(COLOR_RED));
    mvaddstr(10, 18, " GAME OVER ");
    mvaddstr(12, 16, " Press Space to restart ");
    attroff(A_BOLD | COLOR_PAIR(COLOR_RED));
    refresh();
}

static void draw_win(void) {
    clear();
    attron(A_BOLD | COLOR_PAIR(COLOR_YELLOW));
    mvaddstr(8, 16, "  CONGRATULATIONS!  ");
    mvaddstr(10, 14, "  You beat all 10 levels!  ");
    mvaddstr(12, 16, " Press Space to play again  ");
    attroff(A_BOLD | COLOR_PAIR(COLOR_YELLOW));
    refresh();
}

int main(void) {
    initscr();
    cbreak();
    noecho();
    curs_set(0);
    keypad(stdscr, TRUE);
    nodelay(stdscr, TRUE);
    start_color();

    init_pair(COLOR_GREEN, COLOR_GREEN, COLOR_BLACK);
    init_pair(COLOR_RED, COLOR_RED, COLOR_BLACK);
    init_pair(COLOR_BLUE, COLOR_CYAN, COLOR_BLACK);
    init_pair(COLOR_YELLOW, COLOR_YELLOW, COLOR_BLACK);
    init_pair(COLOR_MAGENTA, COLOR_MAGENTA, COLOR_BLACK);
    init_pair(COLOR_CYAN, COLOR_CYAN, COLOR_BLACK);
    init_pair(COLOR_WHITE, COLOR_WHITE, COLOR_BLACK);

    Game game;
    init_game(&game);

    while (1) {
        switch (game.state) {
            case STATE_TITLE:
                draw_title();
                break;
            case STATE_PLAYING:
                move_platforms(&game.levels[game.level]);
                update_mario(&game);
                update_enemies(&game);
                update_coins(&game);
                draw_game(&game);
                break;
            case STATE_LEVEL_DONE:
                draw_game(&game);
                if (--game.level_done_timer <= 0) {
                    draw_level_done(&game);
                }
                break;
            case STATE_GAME_OVER:
                draw_game_over();
                break;
            case STATE_WIN:
                draw_win();
                break;
        }
        usleep(40000);
    }

    endwin();
    return 0;
}
