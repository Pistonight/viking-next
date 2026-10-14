import typing
from argparse import ArgumentParser

from viking import find_tool, get_config


def add_custom_arguments(parser: ArgumentParser):
    _ = parser.add_argument(
        "--version",
        dest="version",
        help="Specify which version should be recompiled on source changes",
    )


using_version = ""


def apply(config: dict[str, typing.Any], args: typing.Any):  # pyright: ignore[reportAny]
    viking_config = get_config()
    root = viking_config.repo.root_path

    version = None
    if isinstance(args, dict):
        version_ = args.get("version")  # pyright: ignore[reportUnknownMemberType, reportUnknownVariableType]
        if version_:
            version = str(version_)  # pyright: ignore[reportUnknownArgumentType]
    else:
        try:
            version = args.version  # pyright: ignore[reportAny]
        except AttributeError:
            pass
    if version is None:
        version = viking_config.repo.default_version

    config["arch"] = "aarch64"
    config["baseimg"] = viking_config.repo.original_elf_with_version(version)
    config["myimg"] = viking_config.repo.decomp_elf_with_version(version)
    source_directories = viking_config.check_mismatch.asm_differ.source_directories
    config["source_directories"] = [str(root / x) for x in source_directories]
    config["objdump_executable"] = find_tool("llvm-objdump")
    # ill-suited to C++ projects (and too slow for large executables)
    config["show_line_numbers_default"] = False

    build_ninja = viking_config.repo.build_ninja_with_version(version)
    config["make_command"] = ["ninja", "-C", str(build_ninja.parent)]

    global using_version
    using_version = version


def map_build_target(make_target: str):
    # this is just mapping the path to relative inside build_dir right (for ninja)??
    viking_config = get_config()
    build_dir = str(viking_config.repo.build_path)
    if using_version:
        versioned_build_dir = str(viking_config.repo.build_path / using_version)
        if make_target.startswith(versioned_build_dir):
            return make_target[len(versioned_build_dir) :].lstrip("/\\")

    if make_target.startswith(build_dir):
        return make_target[len(build_dir) :].lstrip("/\\")

    # if make_target == util.config.get_decomp_elf():
    #     return util.config.get_build_target()

    # TODO: When support for directly diffing object files is added, this needs to strip
    # the build/ prefix from the object file targets.
    return make_target
