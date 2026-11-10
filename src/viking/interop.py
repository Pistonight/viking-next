
import types
import typing
import weakref


class RustObj[T]:
    """An owning reference in Python to a Rust object"""

    def __init__(self, value: T, free: typing.Callable[[T], None] | None):
        self._value: T | None = value
        self._type_name: str = type(value).__name__
        # the finalizer callback must not reference self, only value
        self._finalizer: weakref.finalize[[T], RustObj[T]] | None = (
            weakref.finalize(self, free, value) if free else None
        )

    @property
    def valid(self) -> bool:
        """Check if this handle is valid"""
        return self._value is not None

    @property
    def handle(self) -> T:
        """Get a handle to the inner object for calling Rust functions"""
        if self._value is None:
            raise RuntimeError(
                "Attempting to access RustObj that was already freed or released"
            )
        return self._value

    def free(self) -> None:
        """Free the Rust object now (idempotent)"""
        self._value = None
        if self._finalizer is not None:
            self._finalizer()  # runs at most once

    def take(self) -> T:
        """Give up ownership without freeing, e.g. when passing to a Rust fn that takes ownership"""
        value = self.handle
        if self._finalizer is not None:
            self._finalizer.detach()
        self._value = None
        return value

    def __enter__(self) -> typing.Self:
        return self

    def __exit__(
        self,
        _exc_type: type[BaseException] | None,
        _exc_value: BaseException | None,
        _traceback: types.TracebackType | None,
    ) -> bool:
        self.free()
        return False

    def __copy__(self) -> typing.NoReturn:
        raise TypeError("RustObj cannot be copied")

    def __deepcopy__(self, _memo: dict[int, object]) -> typing.NoReturn:
        raise TypeError("RustObj cannot be copied")

    @typing.override
    def __reduce__(self):
        raise TypeError("RustObj cannot be pickled")

    @typing.override
    def __repr__(self) -> str:
        state = "valid" if self.valid else "freed"
        return f"<RustObj[{self._type_name}] {state}>"
