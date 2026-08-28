/**
 * wbwIME Native API — C 头文件
 *
 * 使用方法：
 *   WbwIme *ime = wbw_ime_create("path/to/dict.cin");
 *   // 或从 .fst 加载：
 *   WbwIme *ime = wbw_ime_create("path/to/dict.fst");
 *
 *   WbwImeResult *result = wbw_ime_process_key(ime, 0, 'a');
 *   // 处理 result ...
 *   wbw_ime_result_free(result);
 *
 *   wbw_ime_destroy(ime);
 */

#ifndef WBW_IME_NATIVE_H
#define WBW_IME_NATIVE_H

#include <stdint.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ========== 不透明指针 ========== */

typedef struct WbwIme WbwIme;

/* ========== 枚举 ========== */

typedef enum {
    WBW_IME_STATE_IDLE      = 0,
    WBW_IME_STATE_INPUTTING = 1,
    WBW_IME_STATE_SELECTING = 2,
    WBW_IME_STATE_CONFIRMING= 3,
    WBW_IME_STATE_ERROR     = 4,
} WbwImeState;

typedef enum {
    WBW_RESPONSE_NONE        = 0,
    WBW_RESPONSE_INPUT_CHAR  = 1,
    WBW_RESPONSE_DELETE_CHAR = 2,
    WBW_RESPONSE_CONFIRM     = 3,
    WBW_RESPONSE_CANCEL      = 4,
    WBW_RESPONSE_SWITCH_MODE = 5,
} WbwImeResponseType;

/* ========== 结构体 ========== */

typedef struct {
    char  *text;    /**< 词文本（UTF-8，需 free） */
    char  *code;    /**< 编码 */
    double score;   /**< 分数 */
} WbwCandidate;

typedef struct {
    uint32_t        response_type;   /**< WbwImeResponseType */
    char           *buffer;          /**< 输入缓冲区（UTF-8） */
    uint32_t        cursor;          /**< 光标位置（字节偏移） */
    WbwCandidate   *candidates;      /**< 候选词列表 */
    uint32_t        candidate_count; /**< 候选词数量 */
    bool            need_refresh;    /**< 是否需要刷新 UI */
    bool            need_hide;       /**< 是否需要隐藏候选窗口 */
    char           *confirmed_text;  /**< 确认的文本（仅 Confirm 类型） */
} WbwImeResult;

/* ========== 生命周期 ========== */

/**
 * 创建 IME 实例
 * @param dict_path 词典文件路径（.cin 或 .fst）
 * @return IME 实例指针，失败返回 NULL
 */
WbwIme *wbw_ime_create(const char *dict_path);

/**
 * 销毁 IME 实例
 * @param ime IME 实例指针
 */
void wbw_ime_destroy(WbwIme *ime);

/* ========== 输入处理 ========== */

/**
 * 处理按键事件
 * @param ime      IME 实例
 * @param key_code 虚拟键码（如 Windows VK_* 或 X11 keysym）
 * @param key_char 字符码（Unicode codepoint，无字符时为 0）
 * @return 结果（需 wbw_ime_result_free 释放）
 */
WbwImeResult *wbw_ime_process_key(WbwIme *ime, uint32_t key_code, uint32_t key_char);

/**
 * 直接输入字符串
 * @param ime  IME 实例
 * @param text 要输入的字符串（UTF-8）
 * @return 结果（需 wbw_ime_result_free 释放）
 */
WbwImeResult *wbw_ime_input_text(WbwIme *ime, const char *text);

/* ========== 查询 ========== */

/**
 * 获取当前 IME 状态
 * @param ime IME 实例
 * @return 当前状态枚举
 */
WbwImeState wbw_ime_get_state(const WbwIme *ime);

/**
 * 重置 IME 状态（清空缓冲区，回到 Idle）
 * @param ime IME 实例
 */
void wbw_ime_reset(WbwIme *ime);

/* ========== 版本 ========== */

/**
 * 获取版本号字符串（需 wbw_ime_string_free 释放）
 * @return 版本号（如 "0.1.0"）
 */
char *wbw_ime_version(void);

/* ========== 内存管理 ========== */

/**
 * 释放 IME 响应结果
 * @param result 由 wbw_ime_process_key 或 wbw_ime_input_text 返回的结果
 */
void wbw_ime_result_free(WbwImeResult *result);

/**
 * 释放 C 字符串
 * @param s 由本模块函数返回的字符串
 */
void wbw_ime_string_free(char *s);

#ifdef __cplusplus
}
#endif

#endif /* WBW_IME_NATIVE_H */
