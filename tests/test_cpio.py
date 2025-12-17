#!/usr/bin/env python3
import os
import subprocess
import shutil
import tempfile
import unittest
import hashlib
import time

class TestCpio(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        # 创建测试目录和文件
        cls.test_dir = tempfile.mkdtemp()
        cls.archive_file = os.path.join(cls.test_dir, "test.cpio")
        cls.extract_dir = os.path.join(cls.test_dir, "extracted")
        cls.source_dir = os.path.join(cls.test_dir, "source")
        cls.destination_dir = os.path.join(cls.test_dir, "destination")
        
        # 清理可能存在的临时目录
        temp_dirs = [
            cls.extract_dir,
            os.path.join(cls.test_dir, "time_extracted"),
            os.path.join(cls.test_dir, "input_extracted"),
            os.path.join(cls.test_dir, "hardlink_dest"),
            cls.destination_dir
        ]
        
        for temp_dir in temp_dirs:
            if os.path.exists(temp_dir):
                try:
                    shutil.rmtree(temp_dir)
                    print(f"✓ 清理临时目录: {temp_dir}")
                except Exception as e:
                    print(f"警告: 无法删除目录 {temp_dir}: {e}")
        
        # 检查并创建源目录结构
        def create_source_structure():
            """创建源目录结构和测试文件"""
            print(f"创建源目录: {cls.source_dir}")
            os.makedirs(cls.source_dir, exist_ok=True)
            os.makedirs(os.path.join(cls.source_dir, "subdir"), exist_ok=True)
            os.makedirs(os.path.join(cls.source_dir, "subdir", "deepdir"), exist_ok=True)
            
            # 创建测试文件
            test_files = [
                ("file1.txt", "Test content 1"),
                ("file2.txt", "Test content 2"),
                ("subdir/file3.txt", "Test content 3"),
                ("subdir/deepdir/file4.txt", "Test content 4"),
                ("empty.txt", ""),
            ]
            
            for file_path, content in test_files:
                full_path = os.path.join(cls.source_dir, file_path)
                os.makedirs(os.path.dirname(full_path), exist_ok=True)
                with open(full_path, "w") as f:
                    f.write(content)
                print(f"✓ 创建文件: {file_path}")
            
            # 设置文件权限和修改时间
            os.chmod(os.path.join(cls.source_dir, "file1.txt"), 0o644)
            os.chmod(os.path.join(cls.source_dir, "subdir"), 0o755)
            
            # 记录原始修改时间
            cls.original_mtime = os.path.getmtime(os.path.join(cls.source_dir, "file1.txt"))
            print(f"✓ 源目录结构创建完成")
        
        # 检查源目录是否存在且包含所需文件
        required_files = [
            "file1.txt",
            "file2.txt", 
            "subdir/file3.txt",
            "subdir/deepdir/file4.txt",
            "empty.txt"
        ]
        
        source_exists = os.path.exists(cls.source_dir)
        files_exist = all(os.path.exists(os.path.join(cls.source_dir, f)) for f in required_files)
        
        if not source_exists or not files_exist:
            print(f"源目录不存在或文件不完整，重新创建...")
            if source_exists:
                shutil.rmtree(cls.source_dir)
                print(f"✓ 删除不完整的源目录: {cls.source_dir}")
            create_source_structure()
        else:
            print(f"✓ 源目录已存在且完整: {cls.source_dir}")
            # 确保记录修改时间
            cls.original_mtime = os.path.getmtime(os.path.join(cls.source_dir, "file1.txt"))

    @classmethod
    def tearDownClass(cls):
        # 清理测试目录
        shutil.rmtree(cls.test_dir)

    def setUp(self):
        """每个测试方法开始前的设置"""
        # 验证源目录完整性
        required_files = [
            "file1.txt",
            "file2.txt", 
            "subdir/file3.txt",
            "subdir/deepdir/file4.txt",
            "empty.txt"
        ]
        
        if not os.path.exists(self.source_dir):
            print(f"警告: 源目录不存在，重新创建: {self.source_dir}")
            self._recreate_source()
        else:
            missing_files = [f for f in required_files if not os.path.exists(os.path.join(self.source_dir, f))]
            if missing_files:
                print(f"警告: 源目录缺少文件，重新创建: {missing_files}")
                self._recreate_source()

    def _recreate_source(self):
        """重新创建源目录结构"""
        if os.path.exists(self.source_dir):
            shutil.rmtree(self.source_dir)
        
        os.makedirs(self.source_dir)
        os.makedirs(os.path.join(self.source_dir, "subdir"))
        os.makedirs(os.path.join(self.source_dir, "subdir", "deepdir"))
        
        test_files = [
            ("file1.txt", "Test content 1"),
            ("file2.txt", "Test content 2"),
            ("subdir/file3.txt", "Test content 3"),
            ("subdir/deepdir/file4.txt", "Test content 4"),
            ("empty.txt", ""),
        ]
        
        for file_path, content in test_files:
            full_path = os.path.join(self.source_dir, file_path)
            os.makedirs(os.path.dirname(full_path), exist_ok=True)
            with open(full_path, "w") as f:
                f.write(content)
        
        os.chmod(os.path.join(self.source_dir, "file1.txt"), 0o644)
        os.chmod(os.path.join(self.source_dir, "subdir"), 0o755)
        self.original_mtime = os.path.getmtime(os.path.join(self.source_dir, "file1.txt"))

    def test_1_create_archive_copy_out(self):
        """测试1: 创建归档 (copy-out 模式) - find <目录> | cpio -o[选项] > <归档文件>"""
        print("\n=== 测试1: 创建归档 (copy-out 模式) ===")
        
        # 使用find和cpio创建归档
        cmd = f"find {self.source_dir} | cpio -o > {self.archive_file}"
        print(f"执行命令: {cmd}")
        
        result = subprocess.run(cmd, shell=True, capture_output=True, text=True)
        print(f"返回码: {result.returncode}")
        if result.stderr:
            print(f"输出: {result.stderr}")
        
        # 验证归档文件已创建
        self.assertTrue(os.path.exists(self.archive_file), "归档文件应该被创建")
        self.assertGreater(os.path.getsize(self.archive_file), 0, "归档文件应该不为空")
        print(f"归档文件大小: {os.path.getsize(self.archive_file)} 字节")

    def test_2_list_archive_content(self):
        """测试2: 列出归档内容 (copy-in 模式) - cpio -itv < <归档文件>"""
        print("\n=== 测试2: 列出归档内容 (copy-in 模式) ===")
        
        # 先创建归档
        self.test_1_create_archive_copy_out()
        
        # 列出归档内容
        cmd = f"cpio -itv < {self.archive_file}"
        print(f"执行命令: {cmd}")
        
        result = subprocess.run(cmd, shell=True, capture_output=True, text=True)
        print(f"返回码: {result.returncode}")
        print(f"输出内容:\n{result.stdout}")
        
        # 验证输出包含预期的文件
        expected_files = ["file1.txt", "file2.txt", "subdir/file3.txt", "subdir/deepdir/file4.txt", "empty.txt"]
        for file in expected_files:
            self.assertIn(file, result.stdout, f"输出应该包含文件: {file}")

    def test_3_extract_archive(self):
        """测试3: 提取归档内容 (copy-in 模式) - cpio -idmv < <归档文件>"""
        print("\n=== 测试3: 提取归档内容 (copy-in 模式) ===")
        
        # 先创建归档
        self.test_1_create_archive_copy_out()
        
        # 预先删除提取目录（如果存在）
        if os.path.exists(self.extract_dir):
            shutil.rmtree(self.extract_dir)
            print(f"✓ 已删除现有提取目录: {self.extract_dir}")
        
        # 创建提取目录
        os.makedirs(self.extract_dir, exist_ok=True)
        
        # 确保提取目录是空的
        if os.listdir(self.extract_dir):
            print(f"警告: 提取目录不为空，清空内容")
            for item in os.listdir(self.extract_dir):
                item_path = os.path.join(self.extract_dir, item)
                if os.path.isdir(item_path):
                    shutil.rmtree(item_path)
                else:
                    os.remove(item_path)
        
        # 提取归档
        cmd = f"cd {self.extract_dir} && cpio -idmv < {self.archive_file}"
        print(f"执行命令: {cmd}")
        
        result = subprocess.run(cmd, shell=True, capture_output=True, text=True)
        print(f"返回码: {result.returncode}")
        if result.stdout:
            print(f"输出: {result.stdout}")
        if result.stderr:
            print(f"输出: {result.stderr}")
        
        # 验证文件已提取
        source_basename = os.path.basename(self.source_dir)
        extracted_files = [
            os.path.join(self.extract_dir, source_basename, "file1.txt"),
            os.path.join(self.extract_dir, source_basename, "file2.txt"),
            os.path.join(self.extract_dir, source_basename, "subdir/file3.txt"),
            os.path.join(self.extract_dir, source_basename, "subdir/deepdir/file4.txt"),
            os.path.join(self.extract_dir, source_basename, "empty.txt"),
        ]
        
        for file in extracted_files:
            self.assertTrue(os.path.exists(file), f"文件应该被提取: {file}")
            print(f"✓ 文件已提取: {file}")
        
        # 验证文件内容
        with open(extracted_files[0], "r") as f:
            content = f.read()
            self.assertEqual(content, "Test content 1", "文件内容应该正确")
            print(f"✓ 文件内容正确: {content}")

    def test_4_copy_pass_mode(self):
        """测试4: 复制文件到另一个目录 (copy-pass 模式) - cpio -pvd <指定目录>"""
        print("\n=== 测试4: 复制文件到另一个目录 (copy-pass 模式) ===")
        
        # 创建目标目录
        if os.path.exists(self.destination_dir):
            shutil.rmtree(self.destination_dir)
            print(f"✓ 已删除现有目标目录: {self.destination_dir}")
        
        os.makedirs(self.destination_dir, exist_ok=True)
        
        # 使用copy-pass模式复制文件
        cmd = f"find {self.source_dir} -print | cpio -pvd {self.destination_dir}"
        print(f"执行命令: {cmd}")
        
        result = subprocess.run(cmd, shell=True, capture_output=True, text=True)
        print(f"返回码: {result.returncode}")
        if result.stdout:
            print(f"输出: {result.stdout}")
        if result.stderr:
            print(f"输出: {result.stderr}")
        
        # 验证文件已复制
        source_basename = os.path.basename(self.source_dir)
        copied_files = [
            os.path.join(self.destination_dir, source_basename, "file1.txt"),
            os.path.join(self.destination_dir, source_basename, "file2.txt"),
            os.path.join(self.destination_dir, source_basename, "subdir/file3.txt"),
            os.path.join(self.destination_dir, source_basename, "subdir/deepdir/file4.txt"),
            os.path.join(self.destination_dir, source_basename, "empty.txt"),
        ]
        
        for file in copied_files:
            self.assertTrue(os.path.exists(file), f"文件应该被复制: {file}")
            print(f"✓ 文件已复制: {file}")
        
        # 验证文件内容
        with open(copied_files[0], "r") as f:
            content = f.read()
            self.assertEqual(content, "Test content 1", "复制的文件内容应该正确")
            print(f"✓ 复制的文件内容正确: {content}")

    def test_5_hard_link_mode(self):
        """测试5: 创建硬链接而不是复制文件 (copy-pass 模式) - cpio -plvd <指定目录>"""
        print("\n=== 测试5: 创建硬链接而不是复制文件 (copy-pass 模式) ===")
        
        hardlink_dest = os.path.join(self.test_dir, "hardlink_dest")
        if os.path.exists(hardlink_dest):
            shutil.rmtree(hardlink_dest)
            print(f"✓ 已删除现有硬链接目标目录: {hardlink_dest}")
        
        os.makedirs(hardlink_dest, exist_ok=True)
        
        # 创建硬链接而不是复制文件
        cmd = f"find {self.source_dir} -print | cpio -plvd {hardlink_dest}"
        print(f"执行命令: {cmd}")
        
        result = subprocess.run(cmd, shell=True, capture_output=True, text=True)
        print(f"返回码: {result.returncode}")
        if result.stdout:
            print(f"输出: {result.stdout}")
        if result.stderr:
            print(f"输出: {result.stderr}")
        
        # 验证文件已创建
        source_basename = os.path.basename(self.source_dir)
        linked_file = os.path.join(hardlink_dest, source_basename, "file1.txt")
        self.assertTrue(os.path.exists(linked_file), "硬链接文件应该被创建")
        print(f"✓ 硬链接文件已创建: {linked_file}")
        
        # 验证inode是否相同(硬链接)
        if os.name != 'nt':  # Windows不支持inode
            src_inode = os.stat(os.path.join(self.source_dir, "file1.txt")).st_ino
            dest_inode = os.stat(linked_file).st_ino
            self.assertEqual(src_inode, dest_inode, "硬链接应该共享相同的inode")
            print(f"✓ 硬链接inode验证通过: {src_inode}")

    def test_6_ascii_header_format(self):
        """测试6: 使用ASCII头部格式 - cpio -c"""
        print("\n=== 测试6: 使用ASCII头部格式 ===")
        
        ascii_archive = os.path.join(self.test_dir, "test_ascii.cpio")
        
        # 创建ASCII格式的归档
        cmd = f"find {self.source_dir} | cpio -oc > {ascii_archive}"
        print(f"执行命令: {cmd}")
        
        result = subprocess.run(cmd, shell=True, capture_output=True, text=True)
        print(f"返回码: {result.returncode}")
        
        # 验证归档文件已创建
        self.assertTrue(os.path.exists(ascii_archive), "ASCII格式归档文件应该被创建")
        print(f"✓ ASCII格式归档文件已创建: {ascii_archive}")

    def test_7_preserve_modification_time(self):
        """测试7: 保留修改时间 - cpio -m"""
        print("\n=== 测试7: 保留修改时间 ===")
        
        # 先创建归档
        self.test_1_create_archive_copy_out()
        
        # 创建提取目录
        time_extract_dir = os.path.join(self.test_dir, "time_extracted")
        
        # 预先删除提取目录（如果存在）
        if os.path.exists(time_extract_dir):
            shutil.rmtree(time_extract_dir)
            print(f"✓ 已删除现有时间提取目录: {time_extract_dir}")
        
        os.makedirs(time_extract_dir, exist_ok=True)
        
        # 确保提取目录是空的
        if os.listdir(time_extract_dir):
            print(f"警告: 时间提取目录不为空，清空内容")
            for item in os.listdir(time_extract_dir):
                item_path = os.path.join(time_extract_dir, item)
                if os.path.isdir(item_path):
                    shutil.rmtree(item_path)
                else:
                    os.remove(item_path)
        
        # 提取归档并保留修改时间
        cmd = f"cd {time_extract_dir} && cpio -idmv < {self.archive_file}"
        print(f"执行命令: {cmd}")
        
        result = subprocess.run(cmd, shell=True, capture_output=True, text=True)
        print(f"返回码: {result.returncode}")
        
        # 验证修改时间是否保留
        source_basename = os.path.basename(self.source_dir)
        extracted_file = os.path.join(time_extract_dir, source_basename, "file1.txt")
        
        if os.path.exists(extracted_file):
            extracted_mtime = os.path.getmtime(extracted_file)
            # 允许1秒的误差
            self.assertAlmostEqual(extracted_mtime, self.original_mtime, delta=1.0, 
                                 msg="修改时间应该被保留")
            print(f"✓ 修改时间已保留: {extracted_mtime}")

    def test_8_specific_output_file(self):
        """测试8: 指定输出文件 - cpio -O <归档文件>"""
        print("\n=== 测试8: 指定输出文件 ===")
        
        specific_archive = os.path.join(self.test_dir, "specific.cpio")
        
        # 使用-O选项指定输出文件
        cmd = f"find {self.source_dir} | cpio -o -O {specific_archive}"
        print(f"执行命令: {cmd}")
        
        result = subprocess.run(cmd, shell=True, capture_output=True, text=True)
        print(f"返回码: {result.returncode}")
        
        # 验证归档文件已创建
        self.assertTrue(os.path.exists(specific_archive), "指定的归档文件应该被创建")
        print(f"✓ 指定的归档文件已创建: {specific_archive}")

    def test_9_specific_input_file(self):
        """测试9: 指定输入文件 - cpio -I <归档文件>"""
        print("\n=== 测试9: 指定输入文件 ===")
        
        # 先创建归档
        self.test_1_create_archive_copy_out()
        
        # 创建提取目录
        input_extract_dir = os.path.join(self.test_dir, "input_extracted")
        
        # 预先删除提取目录（如果存在）
        if os.path.exists(input_extract_dir):
            shutil.rmtree(input_extract_dir)
            print(f"✓ 已删除现有输入提取目录: {input_extract_dir}")
        
        os.makedirs(input_extract_dir, exist_ok=True)
        
        # 确保提取目录是空的
        if os.listdir(input_extract_dir):
            print(f"警告: 输入提取目录不为空，清空内容")
            for item in os.listdir(input_extract_dir):
                item_path = os.path.join(input_extract_dir, item)
                if os.path.isdir(item_path):
                    shutil.rmtree(item_path)
                else:
                    os.remove(item_path)
        
        # 使用-I选项指定输入文件
        cmd = f"cd {input_extract_dir} && cpio -idmv -I {self.archive_file}"
        print(f"执行命令: {cmd}")
        
        result = subprocess.run(cmd, shell=True, capture_output=True, text=True)
        print(f"返回码: {result.returncode}")
        
        # 验证文件已提取
        source_basename = os.path.basename(self.source_dir)
        extracted_file = os.path.join(input_extract_dir, source_basename, "file1.txt")
        self.assertTrue(os.path.exists(extracted_file), "文件应该被提取")
        print(f"✓ 文件已提取: {extracted_file}")

    def test_10_verbose_output(self):
        """测试10: 详细输出 - cpio -v"""
        print("\n=== 测试10: 详细输出 ===")
        
        # 先创建归档
        self.test_1_create_archive_copy_out()
        
        # 使用详细输出模式列出归档内容
        cmd = f"cpio -itv < {self.archive_file}"
        print(f"执行命令: {cmd}")
        
        result = subprocess.run(cmd, shell=True, capture_output=True, text=True)
        print(f"返回码: {result.returncode}")
        print(f"详细输出:\n{result.stdout}")
        
        # 验证输出包含详细信息
        self.assertIn("file1.txt", result.stdout, "详细输出应该包含文件名")
        print("✓ 详细输出验证通过")

if __name__ == '__main__':
    unittest.main(verbosity=2)
