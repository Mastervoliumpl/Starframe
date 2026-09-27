using System;
using Microsoft.VisualStudio.TestTools.UnitTesting;
using Starframe.Bootstrap;

namespace Starframe.Runtime.Tests;

[TestClass]
public sealed class MenuLifecycleTests
{
    [TestMethod]
    public void FailedConstructionCleansPartialObjectsAndDoesNotRetryUntilSceneChanges()
    {
        var lifecycle = new MenuLifecycle();
        var sidebar = new object();
        var settings = new object();
        bool partialPage = false;
        int builds = 0;
        void Cleanup() => partialPage = false;
        void Build()
        {
            builds++;
            partialPage = true;
            throw new InvalidOperationException("Missing native control");
        }
        Assert.ThrowsExactly<InvalidOperationException>(() => lifecycle.Build(sidebar, settings, Build, Cleanup));
        Assert.IsFalse(partialPage);
        lifecycle.Build(sidebar, settings, Build, Cleanup);
        Assert.AreEqual(1, builds);
        lifecycle.Build(sidebar, new object(), () => partialPage = true, Cleanup);
        Assert.IsTrue(partialPage);
    }

    [TestMethod]
    public void ReplacingEitherNativeOwnerCleansPreviousPageBeforeRebuilding()
    {
        var lifecycle = new MenuLifecycle();
        var sidebar = new object();
        var settings = new object();
        bool page = false;
        int builds = 0;
        void Build() { Assert.IsFalse(page); page = true; builds++; }
        void Cleanup() => page = false;
        lifecycle.Build(sidebar, settings, Build, Cleanup);
        lifecycle.Build(sidebar, settings, Build, Cleanup);
        lifecycle.Build(new object(), settings, Build, Cleanup);
        Assert.AreEqual(2, builds);
    }

    [TestMethod]
    public void NativeAndUnityEscapeHandlersBackOutOnlyOnceInEitherOrder()
    {
        foreach (bool nativeFirst in new[] { true, false })
        {
            var lifecycle = new MenuLifecycle();
            int depth = 2;
            void Back() => depth--;
            void Native()
            {
                if (lifecycle.HandledEscape(10)) return;
                lifecycle.Escape(10, Back);
            }
            if (nativeFirst) { Native(); lifecycle.Escape(10, Back); }
            else { lifecycle.Escape(10, Back); Native(); }
            Assert.AreEqual(1, depth);
            lifecycle.Escape(11, Back);
            Assert.AreEqual(0, depth);
            Assert.IsTrue(lifecycle.HandledEscape(11));
            Assert.IsFalse(lifecycle.HandledEscape(12));
        }
    }
}
