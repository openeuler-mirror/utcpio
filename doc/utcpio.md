/*
 * SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

`utcpio`（复制进/出）是一个用于创建和提取归档文件的命令行工具。

以下是 `utcpio` 的主要用法：

**1. 创建归档 (copy-out 模式)**

   * **语法:** `find <目录> | utcpio -o[选项] > <归档文件>`
   * **示例:**
      ```bash
      find ./mydir | utcpio -o > myarchive.utcpio
      ```
      这将把 `mydir` 目录及其所有子目录和文件打包到 `myarchive.utcpio` 文件中。

**2. 列出归档内容 (copy-in 模式)**

   * **语法:** `utcpio -itv < <归档文件>`
   * **示例:**
      ```bash
      utcpio -itv < myarchive.utcpio
      ```
      这将列出 `myarchive.utcpio` 文件的内容，包括文件名、权限和大小。

**3. 提取归档内容 (copy-in 模式)**

   * **语法:** `utcpio -idmv < <归档文件>`
   * **示例:**
      ```bash
      utcpio -idmv < myarchive.utcpio
      ```
      这将把 `myarchive.utcpio` 文件中的内容提取到当前目录。

**4. 复制文件到另一个目录 (copy-pass 模式)**
   * **语法:** `utcpio --pvd  <指定目录>`
   * **示例:**
      ```bash
      find . -print | utcpio -pvd /path/to/destination      
      ```
**5. 创建硬链接而不是复制文件 (copy-pass 模式)**
   * **语法:** `utcpio --plvd  <指定目录>`
   * **示例:**
      ```bash
      find . -print | utcpio -plvd /path/to/destination
      ```

**常用选项：**

* **-o (copy-out):** 创建归档。
* **-i (copy-in):** 提取归档。
* **-t (list):** 列出归档内容。
* **-v (verbose):** 显示详细输出。
* **-d (make directories):** 必要时创建目录。
* **-m (preserve modification time):** 保留文件的修改时间。
* **-c (ASCII header):** 使用 ASCII 头部，提高可移植性。
* **-O <归档文件> (output file):** 指定输出归档文件。
* **-I <归档文件> (input file):** 指定输入归档文件。
* **-F <归档文件> :** 指定要提取的归档文件。



**注意事项：**

* `utcpio` 通常与 `find` 命令结合使用，以指定要归档的文件。
* `utcpio` 不会自动压缩归档文件，需要使用 `gzip` 或其他压缩工具。
* `utcpio` 默认情况下不会覆盖已存在的文件，可以使用 `-u` 选项覆盖。

**优点：**

* `utcpio` 可以处理任何类型的文件，包括设备文件和符号链接。
* `utcpio` 可以跨越多个磁盘或磁带。
* `utcpio` 可以创建增量备份。

