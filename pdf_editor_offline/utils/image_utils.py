from typing import Tuple

from PIL import Image


def resize_image(input_path: str, output_path: str, width: int, height: int) -> None:
    """
    Resize an image to the specified dimensions.

    Args:
        input_path (str): Path to the input image.
        output_path (str): Path to save the resized image.
        width (int): New width.
        height (int): New height.
    """
    with Image.open(input_path) as img:
        resized = img.resize((width, height), Image.Resampling.LANCZOS)
        resized.save(output_path)


def convert_format(input_path: str, output_path: str, format: str) -> None:
    """
    Convert an image to a different format.

    Args:
        input_path (str): Path to the input image.
        output_path (str): Path to save the converted image.
        format (str): Target format (e.g., 'JPEG', 'PNG').
    """
    with Image.open(input_path) as img:
        img.save(output_path, format.upper())


def get_image_size(image_path: str) -> Tuple[int, int]:
    """
    Get the size (width, height) of an image.

    Args:
        image_path (str): Path to the image.

    Returns:
        Tuple[int, int]: Width and height.
    """
    with Image.open(image_path) as img:
        return img.size
