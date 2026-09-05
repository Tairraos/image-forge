// ESLint 扁平化配置（Flat Config）
// 统一检查前端 Vue/JS 源码、测试与脚本，忽略构建产物与 Rust 端。

import js from '@eslint/js';
import pluginVue from 'eslint-plugin-vue';
import prettier from 'eslint-config-prettier';
import globals from 'globals';

export default [
  // 全局忽略
  {
    ignores: [
      'dist/**',
      'node_modules/**',
      'src-tauri/**',
      'release/**',
      'coverage/**',
      '*.min.js',
    ],
  },

  // 基础规则（所有 JS/Vue 文件）
  js.configs.recommended,

  // Vue 3 推荐规则（eslint-plugin-vue 10.x 的 flat config 键名）
  ...pluginVue.configs['flat/recommended'],

  // 关闭与 Prettier 冲突的规则（必须放最后）
  prettier,

  {
    files: ['src/**/*.{js,vue}', 'tests/**/*.js', 'vite.config.js', 'dev-server/**/*.mjs'],
    languageOptions: {
      ecmaVersion: 2022,
      sourceType: 'module',
      globals: {
        ...globals.browser,
        ...globals.node,
      },
    },
  },

  {
    files: ['scripts/**/*.mjs'],
    languageOptions: {
      ecmaVersion: 2022,
      sourceType: 'module',
      globals: {
        ...globals.node,
      },
    },
  },

  // 针对测试文件放宽一些限制
  {
    files: ['tests/**/*.js'],
    languageOptions: {
      globals: {
        ...globals.browser,
        ...globals.node,
        ...globals.vitest,
      },
    },
  },

  // 项目级规则微调
  {
    files: ['src/**/*.{js,vue}'],
    rules: {
      // 单文件组件允许单名单词（App.vue、main.js 等）
      'vue/multi-word-component-names': 'off',
      // 未使用变量只警告，不阻断（vibe coding 迭代快，避免卡住）；
      // ignoreRestSiblings：`({ dataUrl, ...attachment })` 这种用 rest 省略字段的写法不算未使用
      'no-unused-vars': [
        'warn',
        { argsIgnorePattern: '^_', varsIgnorePattern: '^_', ignoreRestSiblings: true },
      ],
      // 允许 console 输出（脚本和调试用）
      'no-console': 'off',
    },
  },

  // v-html 为有意使用的文件：
  // - AgentMessageList：markdown-it 渲染（html:false，原始 HTML 已转义）
  // - TemplateEditorDialog：模板高亮（先 escapeHtml 再包 span）
  // - AppIcon：内部静态 SVG 图标，无外部输入
  {
    files: [
      'src/components/AgentMessageList.vue',
      'src/components/dialogs/TemplateEditorDialog.vue',
      'src/components/snippets/AppIcon.vue',
    ],
    rules: {
      'vue/no-v-html': 'off',
    },
  },
];
